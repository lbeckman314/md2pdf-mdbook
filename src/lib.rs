extern crate html2md;
extern crate regex;
use html2md::parse_html;
use inflector::cases::kebabcase::to_kebab_case;
#[macro_use]
extern crate log;
extern crate env_logger;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use regex::Regex;
use std::default::Default;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::string::String;
use tiny_skia::Pixmap;
use walkdir::WalkDir;

/// TODO https://github.com/raphlinus/pulldown-cmark/blob/master/src/html.rs

/// Used to keep track of current pulldown_cmark "event".
#[derive(Debug)]
enum EventType {
    Code,
    Emphasis,
    Header,
    Html,
    Strong,
    Table,
    TableHead,
    Text,
}

pub struct CurrentType {
    event_type: EventType,
}

pub struct Converter<'a> {
    content: &'a str,
    template: Option<&'a str>,
    assets: Option<&'a str>,
}

impl<'a> Converter<'a> {
    pub fn new(content: &'a str) -> Converter<'a> {
        Converter {
            content: content,
            template: None,
            assets: None,
        }
    }

    pub fn template(self, template: &'a str) -> Converter {
        Converter {
            template: Some(template),
            ..self
        }
    }

    pub fn assets(self, assets: &'a str) -> Converter {
        Converter {
            assets: Some(assets),
            ..self
        }
    }
}

/// Backwards-compatible function.
pub fn markdown_to_tex(content: String) -> String {
    convert(&content, None)
}

pub fn md_to_tex(converter: Converter) -> String {
    let latex = convert(converter.content, converter.assets);

    let mut output = String::new();
    match converter.template {
        Some(template) => {
            output.push_str(&template);
            // Insert new LaTeX data into template after "\begin{document}".
            let mark = "\\begin{document}";
            let pos = template.find(&mark).unwrap() + mark.len();
            output.insert_str(pos, &latex);
        }
        None => output.push_str(&latex),
    }

    output
}

/// Converts markdown string to tex string.
fn convert(content: &str, assets_prefix: Option<&str>) -> String {
    let mut output = String::new();

    let mut header_value = String::new();

    let mut current: CurrentType = CurrentType {
        event_type: EventType::Text,
    };
    let mut cells = 0;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(&content, options);

    let mut equation_mode = false;
    let mut buffer = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading(level)) => {
                current.event_type = EventType::Header;
                output.push_str("\n");
                output.push_str("\\");
                match level {
                    // -1 => output.push_str("part{"),
                    0 => output.push_str("chapter{"),
                    1 => output.push_str("section{"),
                    2 => output.push_str("subsection{"),
                    3 => output.push_str("subsubsection{"),
                    4 => output.push_str("paragraph{"),
                    5 => output.push_str("subparagraph{"),
                    _ => error!("header is out of range."),
                }
            }
            Event::End(Tag::Heading(_)) => {
                output.push_str("}\n");
                output.push_str("\\");
                output.push_str("label{");
                output.push_str(&header_value);
                output.push_str("}\n");

                output.push_str("\\");
                output.push_str("label{");
                output.push_str(&to_kebab_case(&header_value));
                output.push_str("}\n");
            }
            Event::Start(Tag::Emphasis) => {
                current.event_type = EventType::Emphasis;
                output.push_str("\\emph{");
            }
            Event::End(Tag::Emphasis) => output.push_str("}"),

            Event::Start(Tag::Strong) => {
                current.event_type = EventType::Strong;
                output.push_str("\\textbf{");
            }
            Event::End(Tag::Strong) => output.push_str("}"),

            Event::Start(Tag::List(None)) => output.push_str("\\begin{itemize}\n"),
            Event::End(Tag::List(None)) => output.push_str("\\end{itemize}\n"),

            Event::Start(Tag::List(Some(_))) => output.push_str("\\begin{enumerate}\n"),
            Event::End(Tag::List(Some(_))) => output.push_str("\\end{enumerate}\n"),

            Event::Start(Tag::Paragraph) => {
                output.push_str("\n");
            }

            Event::End(Tag::Paragraph) => {
                // ~ adds a space to prevent
                // "There's no line here to end" error on empty lines.
                output.push_str(r"~\\");
                output.push_str("\n");
            }

            Event::Start(Tag::Link(_, url, _)) => {
                // URL link (e.g. "https://nasa.gov/my/cool/figure.png")
                if url.starts_with("http") {
                    output.push_str("\\href{");
                    output.push_str(&*url);
                    output.push_str("}{");
                // local link (e.g. "my/cool/figure.png")
                } else {
                    output.push_str("\\hyperref[");
                    let mut found = false;

                    // iterate through `src` directory to find the resource.
                    let current = std::env::current_dir().unwrap();
                    let src = current.parent();
                    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
                        let _path = entry.path().to_str().unwrap();
                        let _url = &url.clone().into_string().replace("../", "");
                        if _path.ends_with(_url) {
                            match fs::File::open(_path) {
                                Ok(_) => (),
                                Err(_) => panic!("Unable to read title from {}", _path),
                            };

                            found = true;
                            break;
                        }
                    }

                    if !found {
                        output.push_str(&*url.replace("#", ""));
                    }

                    output.push_str("]{");
                }
            }

            Event::End(Tag::Link(_, _, _)) => {
                output.push_str("}");
            }

            Event::Start(Tag::Table(_)) => {
                current.event_type = EventType::Table;
                let table_start = vec![
                    "\n",
                    r"\begingroup",
                    r"\setlength{\LTleft}{-20cm plus -1fill}",
                    r"\setlength{\LTright}{\LTleft}",
                    r"\begin{longtable}{!!!}",
                    r"\hline",
                    r"\hline",
                    "\n",
                ];
                for element in table_start {
                    output.push_str(element);
                    output.push_str("\n");
                }
            }

            Event::Start(Tag::TableHead) => {
                current.event_type = EventType::TableHead;
            }

            Event::End(Tag::TableHead) => {
                output.truncate(output.len() - 2);
                output.push_str(r"\\");
                output.push_str("\n");

                output.push_str(r"\hline");
                output.push_str("\n");

                // we presume that a table follows every table head.
                current.event_type = EventType::Table;
            }

            Event::End(Tag::Table(_)) => {
                let table_end = vec![
                    r"\arrayrulecolor{black}\hline",
                    r"\end{longtable}",
                    r"\endgroup",
                    "\n",
                ];

                for element in table_end {
                    output.push_str(element);
                    output.push_str("\n");
                }

                let mut cols = String::new();
                for _i in 0..cells {
                    cols.push_str(&format!(
                        r"C{{{width}\textwidth}} ",
                        width = 1. / cells as f64
                    ));
                }
                output = output.replace("!!!", &cols);
                cells = 0;
                current.event_type = EventType::Text;
            }

            Event::Start(Tag::TableCell) => match current.event_type {
                EventType::TableHead => {
                    output.push_str(r"\bfseries{");
                }
                _ => (),
            },

            Event::End(Tag::TableCell) => {
                match current.event_type {
                    EventType::TableHead => {
                        output.push_str(r"}");
                        cells += 1;
                    }
                    _ => (),
                }

                output.push_str(" & ");
            }

            Event::Start(Tag::TableRow) => {
                current.event_type = EventType::Table;
            }

            Event::End(Tag::TableRow) => {
                output.truncate(output.len() - 2);
                output.push_str(r"\\");
                output.push_str(r"\arrayrulecolor{lightgray}\hline");
                output.push_str("\n");
            }

            Event::Start(Tag::Image(_, path, title)) => {
                let mut assets_path = String::new();
                match assets_prefix {
                    Some(assets_prefix) => assets_path.push_str(&assets_prefix),
                    None => (),
                }
                assets_path.push_str(&path.clone().into_string());

                // if image path ends with ".svg", run it through
                // svg2png to convert to png file.
                if get_extension(&path).unwrap() == "svg" {
                    let img = svg2png(assets_path);

                    let mut filename_png = String::from(path.clone().into_string());
                    filename_png = filename_png.replace(".svg", ".png");
                    filename_png = filename_png.replace("../../", "");

                    // create output directories.
                    let _ = fs::create_dir_all(Path::new(&filename_png).parent().unwrap());

                    img.save_png(std::path::Path::new(&filename_png)).unwrap();
                    assets_path = filename_png.clone();
                }

                output.push_str("\\begin{figure}\n");
                output.push_str("\\centering\n");
                output.push_str("\\includegraphics[width=\\textwidth]{");
                output.push_str(&assets_path);
                output.push_str("}\n");
                output.push_str("\\caption{");
                output.push_str(&*title);
                output.push_str("}\n\\end{figure}\n");
            }

            Event::Start(Tag::Item) => output.push_str("\\item "),
            Event::End(Tag::Item) => output.push_str("\n"),

            Event::Start(Tag::CodeBlock(lang)) => {
                let re = Regex::new(r",.*").unwrap();
                current.event_type = EventType::Code;
                match lang {
                    CodeBlockKind::Indented => {
                        output.push_str("\\begin{lstlisting}\n");
                    }
                    CodeBlockKind::Fenced(lang) => {
                        output.push_str("\\begin{lstlisting}[language=");
                        output.push_str(&re.replace(&lang, ""));
                        output.push_str("]\n");
                    }
                }
            }

            Event::End(Tag::CodeBlock(_)) => {
                output.push_str("\n\\end{lstlisting}\n");
                current.event_type = EventType::Text;
            }

            Event::Code(t) => {
                output.push_str("\\lstinline|");
                match current.event_type {
                    EventType::Header => output
                        .push_str(&*t.replace("#", r"\#").replace("…", "...").replace("З", "3")),
                    _ => output
                        .push_str(&*t.replace("…", "...").replace("З", "3").replace("�", r"\�")),
                }
                output.push_str("|");
            }

            Event::Html(t) => {
                current.event_type = EventType::Html;
                // convert common html patterns to tex
                output.push_str(convert(&parse_html(&t.into_string()), assets_prefix).as_str());
                current.event_type = EventType::Text;
            }

            Event::Text(t) => {
                // if "\(" or "\[" are encountered, then begin equation
                // and don't replace any characters.
                let delim_start = vec![r"\(", r"\["];
                let delim_end = vec![r"\)", r"\]"];

                if buffer.len() > 100 {
                    buffer.clear();
                }

                buffer.push_str(&t.clone().into_string());

                match current.event_type {
                    EventType::Strong
                    | EventType::Emphasis
                    | EventType::Text
                    | EventType::Header => {
                        // TODO more elegant way to do ordered `replace`s (structs?).
                        if delim_start
                            .into_iter()
                            .any(|element| buffer.contains(element))
                        {
                            let popped = output.pop().unwrap();
                            if popped != '\\' {
                                output.push(popped);
                            }
                            output.push_str(&*t);
                            equation_mode = true;
                        } else if delim_end
                            .into_iter()
                            .any(|element| buffer.contains(element))
                            || equation_mode == true
                        {
                            let popped = output.pop().unwrap();
                            if popped != '\\' {
                                output.push(popped);
                            }
                            output.push_str(&*t);
                            equation_mode = false;
                        } else {
                            output.push_str(
                                &*t.replace(r"\", r"\\")
                                    .replace("&", r"\&")
                                    .replace(r"\s", r"\textbackslash{}s")
                                    .replace(r"\w", r"\textbackslash{}w")
                                    .replace("_", r"\_")
                                    .replace(r"\<", "<")
                                    .replace(r"%", "%")
                                    .replace(r"$", r"\$")
                                    .replace(r"—", "---")
                                    .replace("#", r"\#"),
                            );
                        }
                        header_value = t.into_string();
                    }
                    _ => output.push_str(&*t),
                }
            }

            Event::SoftBreak => {
                output.push('\n');
            }

            Event::HardBreak => {
                output.push_str(r"\\");
                output.push('\n');
            }

            _ => (),
        }
    }

    output
}

/// Simple HTML parser.
///
/// Eventually I hope to use a mature 'HTML to tex' parser.
/// Something along the lines of https://github.com/Adonai/html2md/
pub fn html2tex(html: String, current: &CurrentType, assets_prefix: Option<&str>) -> String {
    let mut tex = html;
    let mut output = String::new();

    // remove all "class=foo" and "id=bar".
    let re = Regex::new(r#"\s(class|id)="[a-zA-Z0-9-_]*">"#).unwrap();
    tex = re.replace(&tex, "").to_string();

    // image html tags
    if tex.contains("<img") {
        // Regex doesn't yet support look aheads (.*?), so we'll use simple pattern matching.
        // let src = Regex::new(r#"src="(.*?)"#).unwrap();
        let src = Regex::new(r#"src="([a-zA-Z0-9-/_.]*)"#).unwrap();
        let caps = src.captures(&tex).unwrap();
        let path_raw = caps.get(1).unwrap().as_str();
        let mut path = String::new();

        match assets_prefix {
            Some(assets_prefix) => path.push_str(&assets_prefix),
            None => (),
        }
        path.push_str(path_raw);

        // if path ends with ".svg", run it through
        // svg2png to convert to png file.
        if get_extension(&path).unwrap() == "svg" {
            let img = svg2png(path.to_string());
            path = path.replace(".svg", ".png");
            path = path.replace("../../", "");

            // create output directories.
            let _ = fs::create_dir_all(Path::new(&path).parent().unwrap());

            img.save_png(std::path::Path::new(&path)).unwrap();
        }

        match current.event_type {
            EventType::Table => {
                output.push_str(r"\begin{center}\includegraphics[width=0.2\textwidth]{")
            }
            _ => {
                output.push_str(r"\begin{center}\includegraphics[width=0.8\textwidth]{");
            }
        }

        output.push_str(&path);
        output.push_str(r"}\end{center}");
        output.push_str("\n");

    // all other tags
    } else {
        match current.event_type {
            // block code
            EventType::Html => {
                tex = tex
                    .replace("/>", "")
                    .replace("<code class=\"language-", "\\begin{lstlisting}")
                    .replace("</code>", r"\\end{lstlisting}")
                    .replace("<span", "")
                    .replace(r"</span>", "")
            }
            // inline code
            _ => {
                tex = tex
                    .replace("/>", "")
                    .replace("<code\n", "<code")
                    .replace("<code", r"\lstinline|")
                    .replace("</code>", r"|")
                    .replace("<span", "")
                    .replace(r"</span>", "");
            }
        }
        // remove all HTML comments.
        let re = Regex::new(r"<!--.*-->").unwrap();
        output.push_str(&re.replace(&tex, ""));
    }

    output
}

/// Converts an SVG file to a PNG file.
///
/// Example: foo.svg becomes foo.svg.png
pub fn svg2png(filename: String) -> Pixmap {
    debug!("svg2png path: {}", &filename);
    let opt = usvg::Options::default();
    let svg_data = std::fs::read(&filename).unwrap();
    let rtree = usvg::Tree::from_data(&svg_data, &opt.to_ref()).unwrap();

    let pixmap_size = rtree.svg_node().size.to_screen_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    resvg::render(&rtree, usvg::FitTo::Original, pixmap.as_mut()).unwrap();

    pixmap
}

/// Extract extension from filename
pub fn get_extension(filename: &str) -> Option<&str> {
    Path::new(filename).extension().and_then(OsStr::to_str)
}

#[cfg(test)]
mod tests {
    #[test]
    fn svg2png_test() {
        assert!(true)
    }

    #[test]
    fn get_extension_test() {
        assert!(true)
    }
}

///
fn path_adder(content: &str, chapter_path: &Path) -> String {
    let mut output = String::new();
    let mut options = Options::empty();
    let parser = Parser::new_ext(content, options);
    for event in parser {
        match event {
            Event::Start(Tag::Image(_, path, title)) => {
                // TODO Append chapter_path to path.
            }
            _ => (),
        }
    }

    output
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_path_adder() {
        let content = "![foo](./foo.png)";
        let path = Path::new("/home/foo/bar");
        let new_path = path_adder(content, &path);
        assert_eq!(new_path, "![foo])(/home/foo/bar/foo.png)");
    }
}
