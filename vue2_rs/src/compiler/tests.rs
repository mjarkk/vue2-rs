#[cfg(test)]
mod tests {
    use super::super::template::Child;
    use super::super::{Parser, Tag, TagType};

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

    #[test]
    fn empty_template() {
        let result = Parser::parse("").unwrap();

        assert!(result.template.is_none());
        assert!(result.script.is_none());
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn simple_template() {
        let result = Parser::parse("<template><h1>hello !</h1></template>").unwrap();

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
        let result = Parser::parse("<script>export default {}</script>").unwrap();

        assert!(result.template.is_none());
        assert_eq!(
            result.script.as_ref().unwrap().content.string(&result),
            "export default {}"
        );
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn template_with_style() {
        let result = Parser::parse("<style>a {color: red;}</style>").unwrap();

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

        let result = Parser::parse(input).unwrap();

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
        let result = Parser::parse(
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
        let result = Parser::parse(
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
        let result = Parser::parse(
            "<template>
                <div>
                    <h1>idk</h1>
                    <test1/>
                    <test2 />
                    <test3>
                        abc
                        <p>def</p>
                        ghi
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
        assert_eq!(children.len(), 3);

        let abc = unwrap_text_child(&result, &children, 0);
        let (_, inner_p_children) = unwrap_element_child(&children, 1);
        let def = unwrap_text_child(&result, &inner_p_children, 0);
        let ghi = unwrap_text_child(&result, &children, 2);

        assert_eq!(abc.trim(), "abc");
        assert_eq!(def, "def");
        assert_eq!(ghi.trim(), "ghi");
    }
}
