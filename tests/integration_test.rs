use std::fs::read_to_string;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use md2tex::md_to_tex;
use md2tex::Converter;

#[test]
fn integration_test() {
    let content_file = "tests/book.md";
    let content = read_to_string(content_file)
        .expect("Something went wrong reading the file");
    let path = Path::new("tests/book.tex");
    let display = path.display();

    let mut file = match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", display, why.description()),
        Ok(file) => file,
    };

    let template_file = "tests/template.tex";
    let template = read_to_string(template_file)
        .expect("Something went wrong reading the file");

    let converter = Converter::new(&content).template(&template).assets("tests/book/src/");
    let latex = md_to_tex(converter);

    match file.write_all(latex.as_bytes()) {
        Err(why) => panic!("couldn't write to {}: {}", display, why.description()),
        Ok(_) => println!("successfully wrote to {}", display),
    }

    assert!(true);
}

