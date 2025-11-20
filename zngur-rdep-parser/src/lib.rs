use itertools::Itertools;
use std::{fs::read_to_string, iter::Peekable, path::PathBuf, str::Lines};
use winnow::{
    Parser, Result,
    combinator::{alt, delimited, opt, separated},
    token::take_while,
};

pub struct RdepParser<'a> {
    input: Peekable<Lines<'a>>,
    rust_types: Vec<Rtype>,
    cpp_types: Vec<String>,
    rust_fns: Vec<String>,
}

pub struct ParsedTypes {
    pub rust_types: Vec<Rtype>,
    pub cpp_types: Vec<String>,
    pub rust_fns: Vec<String>,
}

impl ParsedTypes {
    pub fn union(&mut self, mut rhs: Self) {}
    fn from_parser(rdp: RdepParser) -> Self {
        Self {
            rust_types: rdp.rust_types,
            cpp_types: rdp.cpp_types,
            rust_fns: rdp.rust_fns,
        }
    }
}

impl<'a> RdepParser<'a> {
    pub fn run(input: PathBuf) -> ParsedTypes {
        let s = read_to_string(input).unwrap();
        let mut p = RdepParser::new(&s);
        ParsedTypes::from_parser(p)
    }

    fn new(input: &'a str) -> Self {
        Self {
            input: input.lines().peekable(),
            rust_types: Vec::new(),
            cpp_types: Vec::new(),
            rust_fns: Vec::new(),
        }
    }
    fn parse(&mut self) {
        // while let Some(i) = self.input.peek() {
        while let Some(mut i) = self.input.next() {
            let Ok(t) = parse_ty(&mut i) else { panic!() };
            self.rust_types.push(t);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rtype {
    base: String,
    generics: Option<Vec<Rtype>>,
    funcs: Option<Vec<String>>,
}

impl Rtype {
    pub fn crayte(&self) -> String {
        self.base.split("::").take(1).collect::<String>()
    }
    pub fn string_output(&self) -> String {
        let mut raw = self.base.clone();
        let Some(g) = &self.generics else {
            return raw;
        };
        let g_formatted = g
            .iter()
            .map(|x| x.string_output())
            .intersperse(", ".to_string())
            .collect::<String>();
        raw.push_str(format!("<{g_formatted}>").as_str());
        raw
    }
}

fn parse_ty<'s>(input: &mut &'s str) -> Result<Rtype> {
    let func_chars = take_while(1.., ('a'..='z', 'A'..='Z', '0'..='9', '_'));
    let type_chars = take_while(1.., ('a'..='z', 'A'..='Z', '0'..='9', '_', ':'));
    let nested = delimited("<", separated(1.., parse_ty, ", "), ">");
    let funcs = delimited("(", separated(1.., alt((func_chars, "*")), ", "), ")");
    (type_chars, opt(nested), opt(funcs))
        .map(
            |(base, generics, funcss): (&str, Option<Vec<Rtype>>, Option<Vec<&str>>)| {
                let funcs = if let Some(f) = funcss {
                    Some(f.into_iter().map(str::to_string).collect_vec())
                } else {
                    None
                };
                Rtype {
                    base: base.to_string(),
                    generics,
                    funcs,
                }
            },
        )
        .parse_next(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn raw_and_crayte_working() {
        let input = "std::Vec<i32>(iter, size)";
        let mut p = RdepParser::new(input);
        p.parse();
        let r = p.rust_types.first().unwrap();
        assert_eq!(r.string_output(), "std::Vec<i32>".to_string());
        assert_eq!(r.crayte(), "std".to_string());
    }

    #[test]
    fn raw_and_crayte_working_complex() {
        let input = "std::HashMap<String, Vec<i32>>(iter, size)";
        let mut p = RdepParser::new(input);
        p.parse();
        let r = p.rust_types.first().unwrap();
        assert_eq!(
            r.string_output(),
            "std::HashMap<String, Vec<i32>>".to_string()
        );
        assert_eq!(r.crayte(), "std".to_string());
    }

    #[test]
    fn working() {
        let input = r#"std::Vec<i32>(iter, size)
std::HashSet<i32>
std::VecDeque<i32>(*)"#;
        let mut p = RdepParser::new(input);
        p.parse();
        assert_eq!(
            p.rust_types,
            vec![
                Rtype {
                    base: "std::Vec".to_string(),
                    generics: Some(vec![Rtype {
                        base: "i32".to_string(),
                        generics: None,
                        funcs: None
                    }]),
                    funcs: Some(vec!["iter".to_string(), "size".to_string()]),
                },
                Rtype {
                    base: "std::HashSet".to_string(),
                    generics: Some(vec![Rtype {
                        base: "i32".to_string(),
                        generics: None,
                        funcs: None
                    }]),
                    funcs: None,
                },
                Rtype {
                    base: "std::VecDeque".to_string(),
                    generics: Some(vec![Rtype {
                        base: "i32".to_string(),
                        generics: None,
                        funcs: None
                    }]),
                    funcs: Some(vec!["*".to_string()]),
                }
            ]
        )
    }

    #[test]
    fn no_namespaces() {
        let input = r#"i32
String"#;
        let mut p = RdepParser::new(input);
        p.parse();
        assert_eq!(
            p.rust_types,
            vec![
                Rtype {
                    base: "i32".to_string(),
                    generics: None,
                    funcs: None,
                },
                Rtype {
                    base: "String".to_string(),
                    generics: None,
                    funcs: None,
                },
            ]
        )
    }

    #[test]
    fn base_only() {
        let input = r#"std::Vec
std::HashSet"#;
        let mut p = RdepParser::new(input);
        p.parse();
        assert_eq!(
            p.rust_types,
            vec![
                Rtype {
                    base: "std::Vec".to_string(),
                    generics: None,
                    funcs: None,
                },
                Rtype {
                    base: "std::HashSet".to_string(),
                    generics: None,
                    funcs: None,
                },
            ]
        )
    }

    #[test]
    fn nesting() {
        let mut input = "<Result<i32>>";
        let v: Rtype = delimited("<", parse_ty, ">")
            .parse_next(&mut input)
            .unwrap();
        assert_eq!(
            v,
            Rtype {
                base: "Result".to_string(),
                generics: Some(vec![Rtype {
                    base: "i32".to_string(),
                    generics: None,
                    funcs: None
                }]),
                funcs: None
            }
        )
    }

    #[test]
    fn base_and_nested() {
        let input = "std::Vec<i32>";
        let mut p = RdepParser::new(input);
        p.parse();
        assert_eq!(
            p.rust_types,
            vec![Rtype {
                base: "std::Vec".to_string(),
                generics: Some(vec![Rtype {
                    base: "i32".to_string(),
                    generics: None,
                    funcs: None
                }]),
                funcs: None
            }]
        )
    }

    #[test]
    fn base_and_multiple_deeply_nested() {
        let input = "std::HashMap<String, Vec<i32>>";
        let mut p = RdepParser::new(input);
        p.parse();
        assert_eq!(
            p.rust_types,
            vec![Rtype {
                base: "std::HashMap".to_string(),
                generics: Some(vec![
                    Rtype {
                        base: "String".to_string(),
                        generics: None,
                        funcs: None
                    },
                    Rtype {
                        base: "Vec".to_string(),
                        generics: Some(vec![Rtype {
                            base: "i32".to_string(),
                            generics: None,
                            funcs: None
                        }]),
                        funcs: None
                    }
                ]),
                funcs: None
            }]
        )
    }
}
