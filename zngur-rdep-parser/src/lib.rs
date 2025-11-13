use std::iter::Peekable;
use winnow::{
    Parser, Result,
    ascii::alphanumeric1,
    combinator::{alt, delimited, opt, separated},
    token::take_while,
};

pub struct RdepParser<'a, T: Iterator<Item = &'a str>> {
    input: Peekable<T>,
    output: Vec<Rtype<'a>>,
}

impl<'a, T: Iterator<Item = &'a str>> RdepParser<'a, T> {
    pub fn new(input: T) -> Self {
        Self {
            input: input.peekable(),
            output: Vec::new(),
        }
    }
    pub fn parse(&mut self) {
        // while let Some(i) = self.input.peek() {
        while let Some(mut i) = self.input.next() {
            let Ok(t) = parse_ty(&mut i) else { panic!() };
            self.output.push(t);
        }
    }
    pub fn output(self) -> Vec<Rtype<'a>> {
        self.output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rtype<'a> {
    base: Vec<&'a str>,
    generics: Option<Vec<Rtype<'a>>>,
    funcs: Option<Vec<&'a str>>,
}

fn parse_ty<'s>(input: &mut &'s str) -> Result<Rtype<'s>> {
    let allowed_func_chars = take_while(1.., ('a'..='z', 'A'..='Z', '0'..='9', '_'));
    let base_ty = separated(1.., alphanumeric1, "::");
    let nested = delimited("<", separated(1.., parse_ty, ", "), ">");
    let funcs = delimited(
        "(",
        separated(1.., alt((allowed_func_chars, "*")), ", "),
        ")",
    );
    (base_ty, opt(nested), opt(funcs))
        .map(|(base, generics, funcs)| Rtype {
            base,
            generics,
            funcs,
        })
        .parse_next(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn working() {
        let input = r#"std::Vec<i32>(iter, size)
std::HashSet<i32>
std::VecDeque<i32>(*)"#;
        let mut p = RdepParser::new(input.lines());
        p.parse();
        assert_eq!(
            p.output,
            vec![
                Rtype {
                    base: vec!["std", "Vec"],
                    generics: Some(vec![Rtype {
                        base: vec!["i32"],
                        generics: None,
                        funcs: None
                    }]),
                    funcs: Some(vec!["iter", "size"]),
                },
                Rtype {
                    base: vec!["std", "HashSet"],
                    generics: Some(vec![Rtype {
                        base: vec!["i32"],
                        generics: None,
                        funcs: None
                    }]),
                    funcs: None,
                },
                Rtype {
                    base: vec!["std", "VecDeque"],
                    generics: Some(vec![Rtype {
                        base: vec!["i32"],
                        generics: None,
                        funcs: None
                    }]),
                    funcs: Some(vec!["*"]),
                }
            ]
        )
    }

    #[test]
    fn no_namespaces() {
        let input = r#"i32
String"#;
        let mut p = RdepParser::new(input.lines());
        p.parse();
        assert_eq!(
            p.output,
            vec![
                Rtype {
                    base: vec!["i32"],
                    generics: None,
                    funcs: None,
                },
                Rtype {
                    base: vec!["String"],
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
        let mut p = RdepParser::new(input.lines());
        p.parse();
        assert_eq!(
            p.output,
            vec![
                Rtype {
                    base: vec!["std", "Vec"],
                    generics: None,
                    funcs: None,
                },
                Rtype {
                    base: vec!["std", "HashSet"],
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
                base: vec!["Result"],
                generics: Some(vec![Rtype {
                    base: vec!["i32"],
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
        let mut p = RdepParser::new(input.lines());
        p.parse();
        assert_eq!(
            p.output,
            vec![Rtype {
                base: vec!["std", "Vec"],
                generics: Some(vec![Rtype {
                    base: vec!["i32"],
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
        let mut p = RdepParser::new(input.lines());
        p.parse();
        assert_eq!(
            p.output,
            vec![Rtype {
                base: vec!["std", "HashMap"],
                generics: Some(vec![
                    Rtype {
                        base: vec!["String"],
                        generics: None,
                        funcs: None
                    },
                    Rtype {
                        base: vec!["Vec"],
                        generics: Some(vec![Rtype {
                            base: vec!["i32"],
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
