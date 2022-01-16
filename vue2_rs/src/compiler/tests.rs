#[cfg(test)]
mod tests {
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
        let result = Parser::parse("<template><h1>hello</h1></template>").unwrap();

        assert_eq!(
            result.template.as_ref().unwrap().content.string(&result),
            "<h1>hello</h1>",
        );
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
        ";

        let result = Parser::parse(input).unwrap();

        assert_eq!(
            result.template.as_ref().unwrap().content.string(&result),
            "<h1>Hello world</h1>"
        );
        assert_eq!(
            result.script.as_ref().unwrap().content.string(&result),
            "export default {}"
        );
        let style_1 = result.styles.get(0).unwrap();
        assert_eq!(style_1.content.string(&result), "h1 {color: red;}");
        assert!(style_1.lang.is_none());
        assert!(style_1.scoped);

        let style_2 = result.styles.get(1).unwrap();
        assert_eq!(style_2.content.string(&result), "h2 {color: red;}");
        assert_eq!(style_2.lang.clone().unwrap().string(&result), "scss");
        assert!(!style_2.scoped);
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
}
