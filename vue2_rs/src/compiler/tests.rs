#[cfg(test)]
mod tests {
    use super::super::template::*;
    use super::super::*;

    fn unwrap_element_child(children: &Vec<Child>, idx: usize) -> (Tag, Vec<Child>) {
        match children.get(idx).unwrap() {
            Child::Tag(tag, children) => (tag.clone(), children.clone()),
            v => panic!("{:?}", v),
        }
    }
    fn unwrap_text_child(parser: &Parser, children: &Vec<Child>, idx: usize) -> String {
        match children.get(idx).unwrap() {
            Child::Text(source_location) => source_location.string(parser),
            v => panic!("{:?}", v),
        }
    }
    fn unwrap_var_child(children: &Vec<Child>, idx: usize) -> SourceLocation {
        match children.get(idx).unwrap() {
            Child::Var(source_location) => source_location.clone(),
            v => panic!("{:?}", v),
        }
    }

    #[test]
    fn empty_template() {
        let result = Parser::new_and_parse("").unwrap();

        assert!(result.template.is_none());
        assert!(result.script.is_none());
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn simple_template() {
        let result = Parser::new_and_parse("<template><h1>hello !</h1></template>").unwrap();

        let template_content = result.template.clone().unwrap().content;
        assert_eq!(template_content.len(), 1);

        let (h1, h1_children) = unwrap_element_child(&template_content, 0);
        assert_eq!(h1.name.string(&result), "h1");
        assert_eq!(h1_children.len(), 1);
        let text = unwrap_text_child(&result, &h1_children, 0);
        assert_eq!(text, "hello !");

        assert!(result.script.is_none());
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn template_with_script() {
        let result = Parser::new_and_parse("<script>export default {}</script>").unwrap();

        assert!(result.template.is_none());
        assert_eq!(
            result.script.as_ref().unwrap().content.string(&result),
            "export default {}"
        );
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn template_with_style() {
        let result = Parser::new_and_parse("<style>a {color: red;}</style>").unwrap();

        assert!(result.template.is_none());
        assert!(result.script.is_none());
        assert_eq!(result.styles.len(), 1);
        assert_eq!(
            result.styles.get(0).unwrap().content.string(&result),
            "a {color: red;}"
        );
    }

    #[test]
    fn filled_template() {
        let input = "
            <template><h1>Hello world</h1></template>

            <script lang='ts'>export default {}</script>

            <style scoped>h1 {color: red;}</style>
            <style lang=scss>h2 {color: red;}</style>
            <style lang=stylus other-arg=\"true\" scoped>h3 {color: blue;}</style>
        ";

        let result = Parser::new_and_parse(input).unwrap();

        assert_eq!(result.template.as_ref().unwrap().content.len(), 1);

        let script = result.script.as_ref().unwrap();
        assert_eq!(script.content.string(&result), "export default {}");
        assert_eq!(script.lang.as_ref().unwrap().string(&result), "ts");
        assert_eq!(
            script
                .default_export_location
                .as_ref()
                .unwrap()
                .string(&result),
            "export default"
        );

        let style_1 = result.styles.get(0).unwrap();
        assert_eq!(style_1.content.string(&result), "h1 {color: red;}");
        assert!(style_1.lang.is_none());
        assert!(style_1.scoped);

        let style_2 = result.styles.get(1).unwrap();
        assert_eq!(style_2.content.string(&result), "h2 {color: red;}");
        assert_eq!(style_2.lang.clone().unwrap().string(&result), "scss");
        assert!(!style_2.scoped);

        let style_3 = result.styles.get(2).unwrap();
        assert_eq!(style_3.content.string(&result), "h3 {color: blue;}");
        assert_eq!(style_3.lang.clone().unwrap().string(&result), "stylus");
        assert!(style_3.scoped);
    }

    #[test]
    fn cannot_have_multiple_templates() {
        let result = Parser::new_and_parse(
            "<template></template>
            <template></template>",
        );

        assert_eq!(
            result.unwrap_err().message,
            "can't have multiple templates in your code"
        );
    }

    #[test]
    fn cannot_have_multiple_scripts() {
        let result = Parser::new_and_parse(
            "<script></script>
            <script></script>",
        );

        assert_eq!(
            result.unwrap_err().message,
            "can't have multiple scripts in your code"
        );
    }

    #[test]
    fn parse_template_content() {
        let result = Parser::new_and_parse(
            "<template>
                <div>
                    <h1>idk</h1>
                    <test1/>
                    <test2 />
                    <test3>
                        abc
                        <p>def</p>
                        ghi
                        {{ jkl }}
                    </test3>
                </div>
            </template>",
        )
        .unwrap();

        let template = result.template.clone().unwrap().content;
        assert_eq!(template.len(), 1);
        let (root_div, root_div_children) = unwrap_element_child(&template, 0);
        assert_eq!(root_div.name.string(&result), "div");
        assert_eq!(root_div_children.len(), 4);

        let (h1, h1_children) = unwrap_element_child(&root_div_children, 0);
        assert_eq!(h1.name.string(&result), "h1");
        assert_eq!(h1_children.len(), 1);
        assert_eq!(unwrap_text_child(&result, &h1_children, 0), "idk");

        let (test1, children) = unwrap_element_child(&root_div_children, 1);
        assert_eq!(test1.name.string(&result), "test1");
        match test1.type_ {
            TagType::OpenAndClose => {}
            v => panic!("{:?}", v),
        }
        assert_eq!(children.len(), 0);

        let (test2, children) = unwrap_element_child(&root_div_children, 2);
        assert_eq!(test2.name.string(&result), "test2");
        match test2.type_ {
            TagType::OpenAndClose => {}
            v => panic!("{:?}", v),
        }
        assert_eq!(children.len(), 0);

        let (test3, children) = unwrap_element_child(&root_div_children, 3);
        assert_eq!(test3.name.string(&result), "test3");
        match test3.type_ {
            TagType::Open => {}
            v => panic!("{:?}", v),
        }
        assert_eq!(children.len(), 4);

        let abc = unwrap_text_child(&result, &children, 0);
        let (_, inner_p_children) = unwrap_element_child(&children, 1);
        let def = unwrap_text_child(&result, &inner_p_children, 0);
        let ghi = unwrap_text_child(&result, &children, 2);
        let jkl_var = unwrap_var_child(&children, 3);

        assert_eq!(abc.trim(), "abc");
        assert_eq!(def, "def");
        assert_eq!(ghi.trim(), "ghi");
        assert_eq!(jkl_var.string(&result), " jkl ");
    }

    #[test]
    fn survive_crappy_template() {
        let cases = vec![
            "<div>",            // only an open tag with no closing tag
            "</div>",           // only a closing tag
            "<div><h1></div>",  // no h1 closing tag, but with with a div closing tag
            "<div><h1></span>", // closing a tag not related to any earlier open tag
            "</div></div>",     // 2 closing tags without open tags
        ];

        for case in cases {
            let testing_code = format!(
                "
                    <template>
                        {}
                    </template>

                    <script>
                        export default {}
                    </script>
               ",
                case, "{}",
            );

            Parser::new_and_parse(&testing_code).unwrap();
        }
    }

    mod js_tests {
        use super::*;

        fn must_parse_js(js: &str, expected_global_vars: Vec<&str>) {
            parse_js(js, expected_global_vars).unwrap();
        }

        fn parse_js(js: &str, expected_global_vars: Vec<&str>) -> Result<(), ParserError> {
            let mut parser = Parser::new(&format!("{}{}", js, "}}"));
            let global_var_locations = js::compile_template_var(&mut parser)?;

            let mut global_var_locations_iter =
                global_var_locations.iter().map(|e| e.string(&parser));
            let mut expected_global_vars_iter = expected_global_vars.iter().map(|e| e.to_string());

            loop {
                match (
                    expected_global_vars_iter.next(),
                    global_var_locations_iter.next(),
                ) {
                    (None, None) => break,
                    (expected, got) => assert_eq!(expected, got),
                }
            }

            let mut global_vars = Vec::new();
            for location in global_var_locations {
                global_vars.push(location.string(&parser));
            }

            Ok(())
        }

        #[test]
        fn var() {
            must_parse_js("count", vec!["count"]);
            must_parse_js("this.count", vec!["this"]);
        }

        #[test]
        fn var_assignment() {
            must_parse_js("count = 1", vec!["count"]);
            must_parse_js("count += 1", vec!["count"]);
            must_parse_js("count -= 1", vec!["count"]);
            must_parse_js("count /= 1", vec!["count"]);
            must_parse_js("count >>= 1", vec!["count"]);
            must_parse_js("count <<= 1", vec!["count"]);

            must_parse_js("foo.bar.baz = 1", vec!["foo"]);
            must_parse_js("foo?.bar?.baz = 1", vec!["foo"]);
            must_parse_js("foo['bar'].baz = 1", vec!["foo"]);
            must_parse_js("foo?.['bar']?.baz = 1", vec!["foo"]);
            must_parse_js("foo['bar']['baz'] = 1", vec!["foo"]);
            must_parse_js("foo?.['bar']?.['baz'] = 1", vec!["foo"]);

            must_parse_js("foo[bar][baz] = 1", vec!["foo", "bar", "baz"]);
            must_parse_js("foo?.[bar]?.[baz] = 1", vec!["foo", "bar", "baz"]);
        }

        #[test]
        fn check() {
            must_parse_js("foo ?? bar", vec!["foo", "bar"]);
            must_parse_js("foo > bar", vec!["foo", "bar"]);
            must_parse_js("foo < bar", vec!["foo", "bar"]);
            must_parse_js("foo == bar", vec!["foo", "bar"]);
            must_parse_js("foo === bar", vec!["foo", "bar"]);
            must_parse_js("foo != bar", vec!["foo", "bar"]);
            must_parse_js("foo !== bar", vec!["foo", "bar"]);
            must_parse_js("foo >= bar", vec!["foo", "bar"]);
            must_parse_js("foo <= bar", vec!["foo", "bar"]);
            must_parse_js("foo ? foo : bar", vec!["foo", "foo", "bar"]);
            must_parse_js("foo || bar", vec!["foo", "bar"]);
            must_parse_js("foo && bar", vec!["foo", "bar"]);

            must_parse_js("this.foo && this.bar", vec!["this", "this"]);
        }
    }
}
