use potassium_oxide::*;


#[test]
fn test_currency() {
    const FIVE: usize = 5;

    let _args: std::fmt::Arguments = usd!(FIVE);
    let five: String = format!("{}", usd!(FIVE));
    let ltrl: &'static str = usd!(5);

    dbg!(&five);
    dbg!(&ltrl);

    assert_eq!(five, "$5");
    assert_eq!(ltrl, "$5");
}
