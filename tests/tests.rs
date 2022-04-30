use k2o::*;


#[test]
fn test_currency() {
    const FIVE: usize = 5;

    let _args: std::fmt::Arguments = money!(FIVE);
    let five: String = format!("{}", money!(FIVE));
    let ltrl: &'static str = money!(5);

    dbg!(&five);
    dbg!(&ltrl);

    assert_eq!(five, "$5");
    assert_eq!(ltrl, "$5");
}


#[test]
fn test_splitter() {
    for (text, n) in [
        ("", 0),
        ("asdf", 1),
        ("asdf qwert", 2),

        ("asdf 'qwert' zxcv", 3),
        ("asdf 'qwert zxcv'", 2),
        ("asdf 'qwert zxcv' yuiop", 3),
        ("asdf 'qwer't zxcv'", 2),
        ("asdf qwe'rt zxcv'", 3),
        ("''asdf' 'qwert zxcv", 3),
        ("''asdf' qwert' zxcv", 3),

        ("asdf qwert; zxcv; yuiop", 2),
        ("asdf 'qwert' 'zxcv'; 'yuiop'", 3),
        ("asdf 'qwert; zxcv;'", 2),

        ("asdf 'qwert a' \"zxcv b\" `yuiop c`", 4),
    ] {
        let (line, cmd) = bot::split_cmd(text);

        assert_eq!(cmd.len(), n);

        eprintln!("{} => {:?} ({})", text, line, cmd.len());
        for word in cmd {
            eprintln!("    {} -> {}", word, bot::unquote(word));
        }
        eprintln!();
    }
}
