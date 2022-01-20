#[cfg(test)]
mod tests {
    use super::super::template::Child;
    use super::super::Parser;

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
        let (h1, h1_children) = match template_content.get(0).unwrap() {
            Child::Tag(tag, children) => (tag, children),
            v => panic!("{:?}", v),
        };
        assert_eq!(h1.name.string(&result), "h1");
        assert_eq!(h1_children.len(), 1);
        let text = match h1_children.get(0).unwrap() {
            Child::Text(t) => t,
            v => panic!("{:?}", v),
        };
        assert_eq!(text.string(&result), "hello !");

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
        let result = Parser::parse("<template><h1>idk</h1></template>").unwrap();

        assert_eq!(result.template.as_ref().unwrap().content.len(), 1);
    }
}
