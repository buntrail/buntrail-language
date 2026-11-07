fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lang = buntrail_language::registry::grammar(&args[1]).unwrap();
    let mut p = tree_sitter::Parser::new();
    p.set_language(&lang).unwrap();
    let t = std::fs::read_to_string(&args[2]).unwrap();
    let tree = p.parse(&t, None).unwrap();
    println!("{}", tree.root_node().to_sexp());
}
