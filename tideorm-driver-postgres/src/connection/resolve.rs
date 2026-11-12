use crate::connection::TableData;
use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::message::{ParameterDescription, RowDescription};
use crate::statement::PgStatementMetadata;
use crate::type_info::{PgCustomType, PgType, PgTypeKind};
use crate::types::Oid;
use crate::HashMap;
use crate::{PgColumn, PgConnection, PgTypeInfo};
use driver_core::column::{ColumnOrigin, TableColumn};
use driver_core::executor::Executor;
use driver_core::sql_str::AssertSqlSafe;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Display;
use std::mem;
use std::ops::ControlFlow;
use std::sync::Arc;

mod pg_catalog;

use pg_catalog::{ColumnResolverRow, TypCategory, TypType, TypeResolverRow};

// NOTE: we should only use raw queries in this module because this may occur in the middle
// of an existing extended query flow. Additionally, some third-party implementations don't
// support named prepared statements, so to execute these statements with the extended query flow,
// we'd have to replace the unnamed prepared statement which is already the one the user wanted
// to execute. This means we'd have to immediately re-prepare it, adding an extra round trip.

impl PgConnection {
    pub(super) async fn resolve_statement_metadata<const QUERIES_ALLOWED: bool>(
        &mut self,
        param_desc: Option<ParameterDescription>,
        row_desc: Option<RowDescription>,
        resolve_column_origin: bool,
    ) -> Result<Arc<PgStatementMetadata>, Error> {
        let param_types = param_desc.map_or_else(Default::default, |desc| desc.types);

        let fields = row_desc.map_or_else(Default::default, |desc| desc.fields);

        if QUERIES_ALLOWED {
            let mut type_resolver = TypeResolver::default();
            let mut column_resolver = ColumnResolver::default();

            for ty in &param_types {
                if self.try_oid_to_type(*ty).is_none() {
                    type_resolver.push_type("NULL", ty.0);
                }
            }

            for field in &fields {
                if self.try_oid_to_type(field.data_type_id).is_none() {
                    type_resolver.push_type("NULL", field.data_type_id.0);
                }

                if let (Some(relation_oid), Some(attribute_no)) =
                    (field.relation_id, field.relation_attribute_no)
                {
                    if resolve_column_origin && !self.has_table_column(relation_oid, attribute_no) {
                        column_resolver.push_column(relation_oid, attribute_no);
                    }
                }
            }

            // No-op if `.push_type()` was not called
            type_resolver.fill_cache(self).await?;

            // No-op if `.push_column()` was not called
            column_resolver.fill_cache(self).await?;
        }

        let mut parameters = Vec::with_capacity(param_types.len());

        for ty in param_types {
            if let Some(type_info) = self.try_oid_to_type(ty) {
                parameters.push(type_info);
            } else {
                parameters.push(PgTypeInfo(PgType::DeclareWithOid(ty)));
            }
        }

        let mut columns = Vec::with_capacity(fields.len());
        let mut column_names = HashMap::with_capacity(fields.len());

        for field in fields {
            let name = UStr::from(field.name);
            let ordinal = columns.len();

            let type_info = self
                .try_oid_to_type(field.data_type_id)
                .unwrap_or(PgTypeInfo(PgType::DeclareWithOid(field.data_type_id)));

            let origin = field.relation_id.zip(field.relation_attribute_no).map_or(
                ColumnOrigin::Expression,
                |(relation_oid, attribue_no)| {
                    self.try_table_column(relation_oid, attribue_no)
                        .map_or(ColumnOrigin::Unknown, ColumnOrigin::Table)
                },
            );

            columns.push(PgColumn {
                ordinal,
                name: name.clone(),
                type_info,
                origin,
                relation_id: field.relation_id,
                relation_attribute_no: field.relation_attribute_no,
            });

            column_names.insert(name, ordinal);
        }

        Ok(Arc::new(PgStatementMetadata {
            columns,
            column_names: column_names.into(),
            parameters,
        }))
    }

    /// Whether this connection can describe every type `parameters` and `rows` name without
    /// asking the server.
    pub(super) fn knows_types(
        &self,
        parameters: Option<&ParameterDescription>,
        rows: Option<&RowDescription>,
    ) -> bool {
        let parameters = parameters.into_iter().flat_map(|desc| &desc.types);
        let columns = rows
            .into_iter()
            .flat_map(|desc| &desc.fields)
            .map(|field| &field.data_type_id);
        parameters
            .chain(columns)
            .all(|&oid| self.try_oid_to_type(oid).is_some())
    }

    fn try_table_column(&self, relation_oid: Oid, attribute_no: i16) -> Option<TableColumn> {
        let table_columns = self.inner.cache_table_data.get(&relation_oid)?;

        let column = table_columns.columns.get(&attribute_no)?;

        Some(TableColumn {
            table: table_columns.table_name.clone(),
            name: column.clone(),
        })
    }

    fn has_table_column(&self, relation_oid: Oid, attribute_no: i16) -> bool {
        self.inner
            .cache_table_data
            .get(&relation_oid)
            .is_some_and(|data| data.columns.contains_key(&attribute_no))
    }

    pub(crate) async fn resolve_types(&mut self, types: &[PgTypeInfo]) -> Result<Vec<Oid>, Error> {
        let mut oids = Vec::with_capacity(types.len());

        let mut unresolved_types = types.iter().peekable();

        // Eagerly try to resolve types, stopping at the first unresolved type
        while let Some(ty) = unresolved_types.peek() {
            let Some(oid) = self.try_type_to_oid(ty) else {
                break;
            };

            oids.push(oid);
            unresolved_types.next();
        }

        // Fast-path: all types resolved
        if oids.len() == types.len() {
            return Ok(oids);
        }

        let mut resolver = TypeResolver::default();

        for ty in unresolved_types.clone() {
            // Skip over subsequent types that are already resolved
            if self.try_type_to_oid(ty).is_some() {
                continue;
            }

            if let PgType::DeclareArrayOf(array_of) = &ty.0 {
                // Eagerly bring the element type into cache for array types declared by-name
                resolver.push_type(
                    format_args!("E'{}'", array_of.elem_name),
                    format_args!("to_regtype(E'{}')", array_of.elem_name),
                );
            }

            resolver.push_type(
                // `escape_default()` should produce a valid SQL string literal
                // https://doc.rust-lang.org/stable/std/primitive.char.html#method.escape_default
                // https://www.postgresql.org/docs/current/sql-syntax-lexical.html#SQL-SYNTAX-STRINGS-ESCAPE
                format_args!("E'{}'", ty.name().escape_default()),
                // `to_regtype()` evaluates to `NULL` if the type does not exist,
                // instead of throwing an exception like `'<name>'::regtype` does.
                format_args!("to_regtype(E'{}')::oid", ty.name().escape_default()),
            );
        }

        resolver.fill_cache(self).await?;

        for ty in unresolved_types {
            oids.push(
                self.try_type_to_oid(ty)
                    .ok_or_else(|| Error::TypeNotFound {
                        type_name: ty.name().to_string(),
                    })?,
            );
        }

        Ok(oids)
    }

    pub(crate) fn try_type_to_oid(&self, ty: &PgTypeInfo) -> Option<Oid> {
        if let Some(oid) = ty.try_oid() {
            return Some(oid);
        }

        match &ty.0 {
            PgType::DeclareWithName(name) => self.inner.cache_type_oid.get(name).copied(),
            PgType::DeclareArrayOf(array) => {
                let typelem = self.inner.cache_type_oid.get(&array.elem_name).copied()?;
                self.inner.cache_elem_type_to_array.get(&typelem).copied()
            }
            // `.try_oid()` should return `Some()` or it should be covered here
            _ => unreachable!("(bug) OID should be resolvable for type {ty:?}"),
        }
    }

    fn try_oid_to_type(&self, oid: Oid) -> Option<PgTypeInfo> {
        PgTypeInfo::try_from_oid(oid).or_else(|| self.inner.cache_type_info.get(&oid).cloned())
    }

    fn try_cache_type(&mut self, ty: &TypeResolverRow) -> Result<ControlFlow<Oid>, Error> {
        if self.try_oid_to_type(ty.oid).is_some() {
            // We hit this code path because one of these names didn't resolve,
            // cache them both.
            self.inner
                .cache_type_oid
                .insert(UStr::new(&ty.catalog_name), ty.oid);
            self.inner
                .cache_type_oid
                .insert(UStr::new(&ty.pretty_name), ty.oid);

            if let Some(original_name) = &ty.original_name {
                self.inner
                    .cache_type_oid
                    .insert(UStr::new(original_name), ty.oid);
            }

            if let Some(elem_oid) = ty.typelem {
                if self.try_oid_to_type(elem_oid).is_some() {
                    self.inner.cache_elem_type_to_array.insert(elem_oid, ty.oid);
                } else {
                    return Ok(ControlFlow::Break(elem_oid));
                }
            }

            return Ok(ControlFlow::Continue(()));
        }

        if self.inner.cache_type_info.contains_key(&ty.oid) {
            return Ok(ControlFlow::Continue(()));
        }

        let custom_type_kind = match (ty.typtype, ty.typcategory) {
            (TypType::Domain, _) => {
                let typbasetype = ty.typbasetype.ok_or_else(|| {
                    err_protocol!(
                        "type category is listed as domain, but no base type was found: {ty:?}"
                    )
                })?;

                let Some(base_type) = self.try_oid_to_type(typbasetype) else {
                    return Ok(ControlFlow::Break(typbasetype));
                };

                PgTypeKind::Domain(base_type)
            }

            (TypType::Base, TypCategory::Array) => {
                let typelem = ty.typelem.ok_or_else(|| {
                    err_protocol!(
                        "type category is listed as array, but no element type was found: {ty:?}"
                    )
                })?;

                let Some(elem_type) = self.try_oid_to_type(typelem) else {
                    return Ok(ControlFlow::Break(typelem));
                };

                self.inner.cache_elem_type_to_array.insert(typelem, ty.oid);

                PgTypeKind::Array(elem_type)
            }

            (TypType::Pseudo, _) => PgTypeKind::Pseudo,

            (TypType::Range | TypType::Multirange, _) => {
                let rngsubtype = ty.rngsubtype.ok_or_else(|| {
                    err_protocol!("range or multirange type has no subtype: {ty:?}")
                })?;

                let Some(sub_type) = self.try_oid_to_type(rngsubtype) else {
                    return Ok(ControlFlow::Break(rngsubtype));
                };

                if ty.typtype == TypType::Multirange {
                    PgTypeKind::Multirange(sub_type)
                } else {
                    PgTypeKind::Range(sub_type)
                }
            }

            (TypType::Enum, _) => PgTypeKind::Enum(ty.enum_labels.iter().cloned().collect()),

            (TypType::Composite, _) => {
                let mut attributes = Vec::with_capacity(ty.record_attributes.len());

                for (name, oid) in &ty.record_attributes {
                    let Some(attribute_type) = self.try_oid_to_type(*oid) else {
                        return Ok(ControlFlow::Break(*oid));
                    };

                    attributes.push((name.clone(), attribute_type));
                }

                PgTypeKind::Composite(attributes.into())
            }

            _ => PgTypeKind::Simple,
        };

        let typname = UStr::new(&ty.pretty_name);

        self.inner
            .cache_type_oid
            .entry_ref(&typname)
            .or_insert(ty.oid);

        if ty.pretty_name != ty.catalog_name {
            self.inner
                .cache_type_oid
                .entry(UStr::new(&ty.catalog_name))
                .or_insert(ty.oid);
        }

        if let Some(original_name) = &ty.original_name {
            self.inner
                .cache_type_oid
                .entry(UStr::new(original_name))
                .or_insert(ty.oid);
        }

        self.inner.cache_type_info.entry(ty.oid).or_insert_with(|| {
            PgTypeInfo(PgType::Custom(Arc::new(PgCustomType {
                kind: custom_type_kind,
                name: typname.clone(),
                oid: ty.oid,
            })))
        });

        Ok(ControlFlow::Continue(()))
    }
}

#[derive(Default)]
struct TypeResolver {
    query: String,
}

impl TypeResolver {
    fn push_type(&mut self, original_name: impl Display, oid_expr: impl Display) {
        use std::fmt::Write;

        tracing::trace!(%original_name, %oid_expr, "push_type");

        // Lazily push the preamble to `self.query` so we don't allocate in the fast path
        // (all types already known)
        if self.query.is_empty() {
            write!(
                &mut self.query,
                // Postgres 13 would return `0` instead of `NULL` for `typelem`, `typbasetype`
                "SELECT pg_type.oid,\n\
                     pg_type.oid::regtype::text pretty_name,\n\
                     typname catalog_name,\n\
                     original_name,\n\
                     typtype,\n\
                     typcategory,\n\
                     NULLIF(typelem, 0::oid) typelem,\n\
                     NULLIF(typbasetype, 0::oid) typbasetype,\n\
                     rngsubtype,\n\
                     COALESCE(\
                        (SELECT array_agg(enumlabel) FROM (SELECT *\n\
                        FROM pg_catalog.pg_enum\n\
                        WHERE enumtypid = pg_type.oid\n\
                        ORDER BY enumsortorder) labels),\n\
                        '{{}}') enum_labels,\n\
                     COALESCE(\n\
                        (SELECT array_agg((attname, atttypid)) FROM (SELECT *\n\
                        FROM pg_catalog.pg_attribute\n\
                        WHERE attrelid = pg_type.typrelid\n\
                            AND NOT attisdropped\n\
                            AND attnum > 0\n\
                        ORDER BY attnum) attributes),\n\
                        '{{}}') record_attributes\n\
                 FROM (SELECT DISTINCT ON(lookup_oid) original_name, lookup_oid\n\
                    FROM (VALUES ({original_name}, {oid_expr})"
            )
            .expect("error writing type expression to query string")
        } else {
            write!(&mut self.query, ", ({original_name}, {oid_expr})")
                .expect("error writing type expression to query string")
        }
    }

    async fn fill_cache(&mut self, conn: &mut PgConnection) -> Result<(), Error> {
        let mut missing_dependencies = HashMap::<Oid, Vec<TypeResolverRow>>::new();

        // Iteratively resolve types until all are resolved, or we hit a dead-end.
        // We statically cap the number of iterations in case we somehow encounter a circular type
        // dependency, which I *assume* Postgres should forbid.
        for _ in 0..64 {
            if self.query.is_empty() {
                break;
            }

            // * Cancel-safety
            // * Makes this type reusable if we want to for whatever reason
            // * Avoids an allocation when converting to `SqlStr`
            let mut query = mem::take(&mut self.query);
            // `pg_range.rngmultitypid` exists from PostgreSQL 14; read through
            // `to_jsonb` it is NULL on older servers instead of an error.
            query.push_str(
                ") lookup_inner(original_name, lookup_oid)\n\
                 ORDER BY lookup_oid) type_lookup\n\
                 INNER JOIN pg_catalog.pg_type ON type_lookup.lookup_oid = pg_type.oid\n\
                 LEFT JOIN pg_catalog.pg_range ON pg_type.oid = pg_range.rngtypid OR \
                     pg_type.oid = (to_jsonb(pg_range)->>'rngmultitypid')::oid",
            );

            tracing::trace!(query, "fill_cache");

            let types = (&mut *conn).fetch_all(AssertSqlSafe(query)).await?;

            'outer: for row in types {
                let mut type_row = TypeResolverRow::from_row(&row)?;

                tracing::trace!("type_row: {type_row:?}");

                let mut resolved_dependencies = VecDeque::new();

                loop {
                    if let ControlFlow::Break(missing_oid) = conn.try_cache_type(&type_row)? {
                        tracing::trace!(
                            ty_name = type_row.catalog_name,
                            missing_oid = missing_oid.0,
                            "type missing dependency"
                        );

                        missing_dependencies
                            .entry(missing_oid)
                            .or_default()
                            .push(type_row);

                        self.push_type("NULL", missing_oid.0);

                        continue 'outer;
                    }

                    resolved_dependencies.extend(
                        missing_dependencies
                            .remove(&type_row.oid)
                            .unwrap_or_default(),
                    );

                    // Iteratively mark existing dependencies as resolved
                    if let Some(next_row) = resolved_dependencies.pop_back() {
                        tracing::trace!(
                            resolved_oid = type_row.oid.0,
                            ty_name = next_row.catalog_name,
                            "resolved dependency"
                        );

                        type_row = next_row
                    } else {
                        break;
                    }
                }
            }
        }

        if !missing_dependencies.is_empty() {
            return Err(Error::Protocol(format!(
                "unable to resolve type OIDs: {:?}",
                missing_dependencies.keys()
            )));
        }

        Ok(())
    }
}

#[derive(Default)]
struct ColumnResolver {
    query: String,
}

impl ColumnResolver {
    fn push_column(&mut self, table_oid: Oid, attribute_no: i16) {
        use std::fmt::Write;

        if self.query.is_empty() {
            write!(
                self.query,
                // Postgres 13 does not accept `(attnum,attname)` without `ROW`
                "SELECT\n\
                    attrelid table_oid,\n\
                    attrelid::regclass::text table_name,\n\
                    array_agg(ROW(attnum, attname)) AS columns\n\
                FROM (VALUES ({}, {attribute_no})",
                table_oid.0,
            )
            .expect("writing to a `String` should be infallible")
        } else {
            write!(self.query, ", ({}, {attribute_no})", table_oid.0)
                .expect("writing to a `String` should be infallible")
        }
    }

    async fn fill_cache(&mut self, conn: &mut PgConnection) -> Result<(), Error> {
        if self.query.is_empty() {
            return Ok(());
        }

        let mut query = mem::take(&mut self.query);
        query.push_str(
            ") lookup(table_oid, attribute_num)\n\
            INNER JOIN pg_catalog.pg_attribute ON lookup.table_oid = attrelid AND lookup.attribute_num = attnum\n\
            GROUP BY attrelid"
        );

        let rows = (&mut *conn).fetch_all(AssertSqlSafe(query)).await?;

        for row in rows {
            let row = ColumnResolverRow::from_row(&row)?;

            let table_columns = conn
                .inner
                .cache_table_data
                .entry(row.table_oid)
                .or_insert_with(|| TableData {
                    table_name: row.table_name.clone(),
                    columns: BTreeMap::new(),
                });

            table_columns.columns.extend(row.columns);
        }

        Ok(())
    }
}
