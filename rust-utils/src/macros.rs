#[macro_export]
macro_rules! matches_opt {
    ($expr:expr, $pattern:pat => $value:expr) => {
        match $expr {
            $pattern => Some($value),
            _ => None,
        }
    };
}

#[test]
fn test_matches_opt() {
    #[derive(Debug, Eq, PartialEq)]
    enum Variant {
        A,
        B,
    }
    let value = matches_opt!(Variant::A, v @ Variant::B => v);
    assert_eq!(value, None);

    let value = matches_opt!(Variant::B, Variant::B => 1);
    assert_eq!(value, Some(1));

    let value = matches_opt!(2, 1..=5 => 2);
    assert_eq!(value, Some(2));

    let value = matches_opt!(2, x @ 0..=1 | x @ 3..=10 => x * 2);
    assert_eq!(value, None);

    let value = matches_opt!(5, x @ 0..=1 | x @ 3..=10 => x * 2);
    assert_eq!(value, Some(10));
}
