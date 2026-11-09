use super::*;
use futures_util::StreamExt;

fn failure() -> Error {
    Error::Protocol("refused".into())
}

/// Every item the stream gives, errors as text, then whether it stays ended.
fn drain(mut stream: TryAsyncStream<'static, u32>) -> (Vec<Result<u32, String>>, bool) {
    crate::rt::test_block_on(async move {
        let mut items = Vec::new();
        while let Some(item) = stream.next().await {
            items.push(item.map_err(|error| error.to_string()));
        }
        (items, stream.next().await.is_none())
    })
}

#[test]
fn streams_yield_in_order_and_end_with_the_block() {
    let refused = Err(failure().to_string());
    let cases: [(&str, TryAsyncStream<'static, u32>, Vec<Result<u32, String>>); 5] = [
        ("nothing", TryAsyncStream::new(|_| async { Ok(()) }), vec![]),
        (
            "values",
            try_stream! {
                for value in 1..=3 {
                    r#yield!(value);
                }
                Ok(())
            },
            vec![Ok(1), Ok(2), Ok(3)],
        ),
        (
            "values with real waits between them",
            try_stream! {
                r#yield!(1);
                crate::rt::yield_now().await;
                crate::rt::yield_now().await;
                r#yield!(2);
                Ok(())
            },
            vec![Ok(1), Ok(2)],
        ),
        (
            "an error at once",
            TryAsyncStream::new(|_| async { Err(failure()) }),
            vec![refused.clone()],
        ),
        (
            "an error after values",
            try_stream! {
                r#yield!(7);
                Err::<(), _>(failure())?;
                r#yield!(8);
                Ok(())
            },
            vec![Ok(7), refused],
        ),
    ];

    for (name, stream, expected) in cases {
        let (items, stays_ended) = drain(stream);
        assert_eq!(items, expected, "{name}");
        assert!(stays_ended, "{name}: polled after its end");
    }
}
