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
            result.template.as_ref().unwrap().string(&result),
            "<h1>hello</h1>",
        );
        assert!(result.script.is_none());
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn template_with_script() {
        let result = Parser::parse("<script>module.exports = {}</script>").unwrap();

        assert!(result.template.is_none());
        assert_eq!(
            result.script.as_ref().unwrap().string(&result),
            "module.exports = {}"
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
            result.styles.get(0).unwrap().string(&result),
            "a {color: red;}"
        );
    }

    #[test]
    fn filled_template() {
        let input = "
            <template><h1>Hello world</h1></template>

            <script>module.exports = {}</script>

            <style scoped>h1 {color: red;}</style>
            <style>h2 {color: red;}</style>
        ";

        let result = Parser::parse(input).unwrap();

        assert_eq!(
            result.template.as_ref().unwrap().string(&result),
            "<h1>Hello world</h1>"
        );
        assert_eq!(
            result.script.as_ref().unwrap().string(&result),
            "module.exports = {}"
        );
        assert_eq!(
            result.styles.get(0).unwrap().string(&result),
            "h1 {color: red;}"
        );
        assert_eq!(
            result.styles.get(1).unwrap().string(&result),
            "h2 {color: red;}"
        );
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
