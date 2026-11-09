use std::panic::catch_unwind;

use rust_decimal::Decimal;

use super::PgMoney;

type Op = fn(PgMoney, PgMoney) -> PgMoney;

const ADD: Op = |a, b| a + b;
const ADD_ASSIGN: Op = |mut a, b| {
    a += b;
    a
};
const SUB: Op = |a, b| a - b;
const SUB_ASSIGN: Op = |mut a, b| {
    a -= b;
    a
};

#[test]
fn money_arithmetic() {
    let cases = [
        ("+", ADD, 1, 2, 3),
        ("+=", ADD_ASSIGN, 1, 2, 3),
        ("-", SUB, 5, 1, 4),
        ("-=", SUB_ASSIGN, 1, 2, -1),
    ];

    for (name, op, a, b, result) in cases {
        assert_eq!(
            op(PgMoney(a), PgMoney(b)),
            PgMoney(result),
            "{a} {name} {b}"
        );
    }
}

#[test]
fn money_arithmetic_panics_on_overflow() {
    let cases = [
        ("+", ADD, i64::MAX, "overflow adding money amounts"),
        ("+=", ADD_ASSIGN, i64::MAX, "overflow adding money amounts"),
        ("-", SUB, i64::MIN, "overflow subtracting money amounts"),
        (
            "-=",
            SUB_ASSIGN,
            i64::MIN,
            "overflow subtracting money amounts",
        ),
    ];

    for (name, op, a, message) in cases {
        let panic = catch_unwind(|| op(PgMoney(a), PgMoney(1))).expect_err(name);
        let text = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied());
        assert_eq!(text, Some(message), "{a} {name} 1");
    }
}

/// A decimal is rescaled to the locale's fraction digits (2 here), keeping its sign.
#[test]
fn money_converts_to_and_from_decimal() {
    assert_eq!(PgMoney(12345).to_decimal(2), Decimal::new(12345, 2));

    // (amount, decimal as mantissa and scale)
    let cases = [
        (12345, (12345, 2)),
        (12345, (123450, 3)),
        (-12345, (-123450, 3)),
        (-12300, (-123, 0)),
    ];

    for (amount, (mantissa, scale)) in cases {
        let decimal = Decimal::new(mantissa, scale);
        assert_eq!(
            PgMoney::from_decimal(decimal, 2),
            PgMoney(amount),
            "{decimal}"
        );
    }
}
