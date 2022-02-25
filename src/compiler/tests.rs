#[cfg(test)]
mod tests {
    use super::super::style;
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
    fn unwrap_var_child(children: &Vec<Child>, idx: usize) -> String {
        match children.get(idx).unwrap() {
            Child::Var(var) => var.clone(),
            v => panic!("{:?}", v),
        }
    }

    #[test]
    fn empty_template() {
        let result = Parser::new_and_parse("", "example").unwrap();

        assert!(result.template.is_none());
        assert!(result.script.is_none());
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn simple_template() {
        let result =
            Parser::new_and_parse("<template><h1>hello !</h1></template>", "example").unwrap();

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
        let result =
            Parser::new_and_parse("<script>export default {}</script>", "example").unwrap();

        assert!(result.template.is_none());
        let script = result.script.as_ref().unwrap();
        assert_eq!(script.content.string(&result), "export default {}");
        assert_eq!(
            script
                .default_export_location
                .as_ref()
                .unwrap()
                .string(&result),
            "export default",
        );
        assert_eq!(result.styles.len(), 0);
    }

    #[test]
    fn template_with_style() {
        let result = Parser::new_and_parse("<style>a {color: red;}</style>", "example").unwrap();

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

        let result = Parser::new_and_parse(input, "example").unwrap();

        assert_eq!(result.template.as_ref().unwrap().content.len(), 1);

        let script = result.script.as_ref().unwrap();
        assert_eq!(script.content.string(&result), "export default {}");
        assert_eq!(
            script
                .default_export_location
                .as_ref()
                .unwrap()
                .string(&result),
            "export default",
        );
        assert_eq!(script.lang.as_ref().unwrap(), "ts");
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
        assert_eq!(style_2.lang.clone().unwrap(), "scss");
        assert!(!style_2.scoped);

        let style_3 = result.styles.get(2).unwrap();
        assert_eq!(style_3.content.string(&result), "h3 {color: blue;}");
        assert_eq!(style_3.lang.clone().unwrap(), "stylus");
        assert!(style_3.scoped);
    }

    #[test]
    fn cannot_have_multiple_templates() {
        let result = Parser::new_and_parse(
            "<template></template>
            <template></template>",
            "example",
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
            "example",
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
            "example",
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
        assert_eq!(jkl_var, " _vm.jkl ");
    }

    #[test]
    fn parse_doc_type() {
        // Doctype above the vue component
        Parser::new_and_parse(
            "<!DOCTYPE html>
            <template>
            </template>",
            "example",
        )
        .unwrap();

        // Doctype within the template
        Parser::new_and_parse(
            "<template>
            <!DOCTYPE html>
            </template>",
            "example",
        )
        .unwrap();
    }

    #[test]
    fn parse_html_comment() {
        // Comment above the vue component
        Parser::new_and_parse(
            "<!-- <template> This should not be parsed </template> -->
            <template>
            </template>",
            "example",
        )
        .unwrap();

        // Comment within the template
        Parser::new_and_parse(
            "<template>
            <!-- <template> This should not be parsed </template> -->
            </template>",
            "example",
        )
        .unwrap();
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

            Parser::new_and_parse(&testing_code, "example").unwrap();
        }
    }

    mod template_to_render_method {
        use super::super::super::template::to_js::children_to_js;
        use super::*;

        fn template_to_js_eq(html: &str, eq: &str) {
            assert_eq!(template_to_js(html), eq);
        }

        fn template_to_js(html: &str) -> String {
            let parser_input = format!("<template>{}</template>", html);
            let result = Parser::new_and_parse(&parser_input, "example").unwrap();
            let template = result.template.as_ref().unwrap();

            let mut resp: Vec<char> = Vec::new();
            children_to_js(&template.content, &result, &mut resp);
            resp.iter().collect()
        }

        #[test]
        fn static_elements_with_content() {
            template_to_js_eq("<div></div>", "_c('div')");
            template_to_js_eq("<div/>", "_c('div')");
            template_to_js_eq("<h1>BOOOO</h1>", "_c('h1',[_vm._v(\"BOOOO\")])");
            template_to_js_eq(
                "<div><h1>BOOOO</h1></div>",
                "_c('div',[_c('h1',[_vm._v(\"BOOOO\")])])",
            );
            template_to_js_eq(
                "<div><h1>BOOOO</h1><p>This is a test</p></div>",
                "_c('div',[_c('h1',[_vm._v(\"BOOOO\")]),_c('p',[_vm._v(\"This is a test\")])])",
            );
        }

        #[test]
        fn vars() {
            template_to_js_eq(
                "<h1>{{ 'hello world' }}</h1>",
                "_c('h1',[_vm._v(_vm._s( 'hello world' ))])",
            );

            template_to_js_eq(
                "<h1>{{ hello_world }}</h1>",
                "_c('h1',[_vm._v(_vm._s( _vm.hello_world ))])",
            );

            template_to_js_eq(
                "<h1>{{ this.hello_world }}</h1>",
                "_c('h1',[_vm._v(_vm._s( _vm.hello_world ))])",
            );

            template_to_js_eq(
                "<h1>foo {{ hello_world }} bar</h1>",
                "_c('h1',[_vm._v(\"foo \"+_vm._s( _vm.hello_world )+\" bar\")])",
            );
        }

        mod args {
            use super::*;

            #[test]
            fn default_args() {
                template_to_js_eq(
                    "<h1 a=b c='d' e>Hmm</h1>",
                    "_c('h1',{attrs:{\"a\":\"b\",\"c\":\"d\",\"e\":true}},[_vm._v(\"Hmm\")])",
                );
            }

            #[test]
            fn v_bind_arg() {
                template_to_js_eq(
                    "<h1 v-bind:value='value'>Hmm</h1>",
                    "_c('h1',{attrs:{\"value\":_vm.value}},[_vm._v(\"Hmm\")])",
                );

                template_to_js_eq(
                    "<custom-component v-bind:value='value'>Hmm</custom-component>",
                    "_c('custom-component',{props:{\"value\":_vm.value}},[_vm._v(\"Hmm\")])",
                );

                template_to_js_eq(
                    "<h1 :value='value'>Hmm</h1>",
                    "_c('h1',{attrs:{\"value\":_vm.value}},[_vm._v(\"Hmm\")])",
                );

                template_to_js_eq(
                    "<custom-component :value='value'>Hmm</custom-component>",
                    "_c('custom-component',{props:{\"value\":_vm.value}},[_vm._v(\"Hmm\")])",
                );
            }

            #[test]
            fn v_on_arg() {
                template_to_js_eq(
                    "<h1 v-on:value='value($event)'>Hmm</h1>",
                    "_c('h1',{on:{\"value\":$event=>{_vm.value($_vm.event)}}},[_vm._v(\"Hmm\")])",
                );

                template_to_js_eq(
                    "<custom-component v-on:value='value($event)'>Hmm</custom-component>",
                    "_c('custom-component',{on:{\"value\":$event=>{_vm.value($_vm.event)}}},[_vm._v(\"Hmm\")])",
                );

                template_to_js_eq(
                    "<h1 @value='value($event)'>Hmm</h1>",
                    "_c('h1',{on:{\"value\":$event=>{_vm.value($_vm.event)}}},[_vm._v(\"Hmm\")])",
                );

                template_to_js_eq(
                    "<custom-component @value='value($event)'>Hmm</custom-component>",
                    "_c('custom-component',{on:{\"value\":$event=>{_vm.value($_vm.event)}}},[_vm._v(\"Hmm\")])",
                );
            }

            #[test]
            fn v_if() {
                template_to_js_eq(
                    "<h1 v-if='value'>WHAA</h1>",
                    "_vm.value?_c('h1',[_vm._v(\"WHAA\")]):_vm._e()",
                );
            }

            #[test]
            fn v_if_else() {
                template_to_js_eq(
                    "<div>
                        <h1 v-if='value'>True</h1>
                        <h1 v-else>False</h1>
                    </div>",
                    "_c('div',[_vm.value?_c('h1',[_vm._v(\"True\")]):_c('h1',[_vm._v(\"False\")])])",
                );
            }

            #[test]
            fn v_if_else_if() {
                template_to_js_eq(
                    "<div>
                        <h1 v-if='value === undefined'>Undefined</h1>
                        <h1 v-else-if='value === true'>True</h1>
                        <h1 v-else-if='value === false'>False</h1>
                    </div>",
                    concat!(
                        "_c('div',[",
                        "_vm.value === undefined",
                        "?_c('h1',[_vm._v(\"Undefined\")])",
                        ":_vm.value === true",
                        "?_c('h1',[_vm._v(\"True\")])",
                        ":_vm.value === false",
                        "?_c('h1',[_vm._v(\"False\")])",
                        ":_vm._e()",
                        "])",
                    ),
                );
            }

            #[test]
            fn v_if_else_if_else() {
                template_to_js_eq(
                    "<div>
                        <h1 v-if='value === undefined'>Undefined</h1>
                        <h1 v-else-if='value === true'>True</h1>
                        <h1 v-else-if='value === false'>False</h1>
                        <h1 v-else>Unknown</h1>
                    </div>",
                    concat!(
                        "_c('div',[",
                        "_vm.value === undefined",
                        "?_c('h1',[_vm._v(\"Undefined\")])",
                        ":_vm.value === true",
                        "?_c('h1',[_vm._v(\"True\")])",
                        ":_vm.value === false",
                        "?_c('h1',[_vm._v(\"False\")])",
                        ":_c('h1',[_vm._v(\"Unknown\")])",
                        "])",
                    ),
                );
            }

            #[test]
            fn v_for() {
                template_to_js_eq(
                    "<div><div v-for='entry in list'/></div>",
                    "_c('div',_vm._l((_vm.list),(entry)=>_c('div')),0)",
                );

                template_to_js_eq(
                    "<div><div v-for='entry in list'>{{ entry }} {{ other_var }}</div></div>",
                    "_c('div',_vm._l((_vm.list),(entry)=>_c('div',[_vm._v(_vm._s( entry )+_vm._s( _vm.other_var ))])),0)",
                );

                // With entry and key
                template_to_js_eq(
                    "<div><div v-for='(entry, key) in list'>{{ entry }} {{ key }}</div></div>",
                    "_c('div',_vm._l((_vm.list),(entry,key)=>_c('div',[_vm._v(_vm._s( entry )+_vm._s( key ))])),0)",
                );

                // With entry, key and index
                template_to_js_eq(
                    "<div><div v-for='(entry, key, index) in list'/></div>",
                    "_c('div',_vm._l((_vm.list),(entry,key,index)=>_c('div')),0)",
                );

                // With custom component
                template_to_js_eq(
                    "<div><custom-component v-for='entry in list'/></div>",
                    "_c('div',_vm._l((_vm.list),(entry)=>_c('custom-component')),1)",
                );

                // With other elements within the same element
                template_to_js_eq(
                    "<div><h1>HELLO</h1><div v-for='entry in list'/></div>",
                    concat!(
                        "_c('div',[",
                        "_c('h1',[_vm._v(\"HELLO\")]),",
                        "_vm._l((_vm.list),(entry)=>_c('div'))",
                        "],2)",
                    ),
                );
            }

            #[test]
            fn v_text() {
                template_to_js_eq(
                    "<div v-text=\"'<div></div>'\" />",
                    "_c('div',{domProps:{\"textContent\":'<div></div>'}})",
                );
            }

            #[test]
            fn v_html() {
                template_to_js_eq(
                    "<div v-html=\"'<div></div>'\" />",
                    "_c('div',{domProps:{\"innerHTML\":'<div></div>'}})",
                );
            }

            #[test]
            fn v_custom_directive() {
                template_to_js_eq(
                    "<div v-show=\"true\" />",
                    "_c('div',{directives:[{name:\"show\",rawName:\"v-show\",value:true,expression:\"true\"}]})",
                );

                template_to_js_eq(
                    "<div v-custom:arg.foo.bar=\"true\" />",
                    "_c('div',{directives:[{name:\"custom\",rawName:\"v-custom:arg.foo.bar\",value:true,expression:\"true\",arg:\"arg\",modifiers:{\"foo\":true,\"bar\":true,}}]})",
                );
            }
        }

        #[test]
        fn template_tag() {
            // empty template tag
            template_to_js_eq("<div><template /></div>", "_c('div',[void 0])");
            template_to_js_eq("<div><template></template></div>", "_c('div',[void 0])");

            // template with tag inside
            template_to_js_eq(
                "<div> <template> <div /> </template> </div>",
                "_c('div',[[_c('div')]])",
            );

            // test template inside of template
            template_to_js_eq(
                "<div><template><template /></template></div>",
                "_c('div',[[void 0]])",
            );

            // test template using v-if
            template_to_js_eq(
                "<div><template v-if='some_var'/></div>",
                "_c('div',[_vm.some_var?void 0:_vm._e()])",
            );

            // test template using v-for
            template_to_js_eq(
                "<div><template v-for='(value,key) in list'>{{value + key}}</template></div>",
                "_c('div',_vm._l((_vm.list),(value,key)=>[_vm._v(_vm._s(value + key))]),0)",
            );
        }
    }

    mod js_tests {
        use super::*;

        fn parse_js(js: &str, expected_global_vars: Vec<&str>, expected_result: &str) {
            let mut parser = Parser::new(&format!("{}{}", js, "}}"));
            let global_var_locations = js::parse_template_var(&mut parser).unwrap();

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
            for location in global_var_locations.iter() {
                global_vars.push(location.string(&parser));
            }

            let js_with_vm_references =
                js::add_vm_references(&parser, &SourceLocation(0, js.len()), &global_var_locations);

            assert_eq!(js_with_vm_references, expected_result);
        }

        #[test]
        fn var() {
            parse_js("count", vec!["count"], "_vm.count");
            parse_js("this.count", vec!["this"], "_vm.count");
        }

        #[test]
        fn var_assignment() {
            parse_js("count = 1", vec!["count"], "_vm.count = 1");
            parse_js("count += 1", vec!["count"], "_vm.count += 1");
            parse_js("count -= 1", vec!["count"], "_vm.count -= 1");
            parse_js("count /= 1", vec!["count"], "_vm.count /= 1");
            parse_js("count >>= 1", vec!["count"], "_vm.count >>= 1");
            parse_js("count <<= 1", vec!["count"], "_vm.count <<= 1");

            parse_js("foo.bar.baz = 1", vec!["foo"], "_vm.foo.bar.baz = 1");
            parse_js("foo?.bar?.baz = 1", vec!["foo"], "_vm.foo?.bar?.baz = 1");
            parse_js("foo['bar'].baz = 1", vec!["foo"], "_vm.foo['bar'].baz = 1");
            parse_js(
                "foo?.['bar']?.baz = 1",
                vec!["foo"],
                "_vm.foo?.['bar']?.baz = 1",
            );
            parse_js(
                "foo['bar']['baz'] = 1",
                vec!["foo"],
                "_vm.foo['bar']['baz'] = 1",
            );
            parse_js(
                "foo?.['bar']?.['baz'] = 1",
                vec!["foo"],
                "_vm.foo?.['bar']?.['baz'] = 1",
            );

            parse_js(
                "foo[bar][baz] = 1",
                vec!["foo", "bar", "baz"],
                "_vm.foo[_vm.bar][_vm.baz] = 1",
            );
            parse_js(
                "foo?.[bar]?.[baz] = 1",
                vec!["foo", "bar", "baz"],
                "_vm.foo?.[_vm.bar]?.[_vm.baz] = 1",
            );
        }

        #[test]
        fn check() {
            parse_js("foo ?? bar", vec!["foo", "bar"], "_vm.foo ?? _vm.bar");
            parse_js("foo > bar", vec!["foo", "bar"], "_vm.foo > _vm.bar");
            parse_js("foo < bar", vec!["foo", "bar"], "_vm.foo < _vm.bar");
            parse_js("foo == bar", vec!["foo", "bar"], "_vm.foo == _vm.bar");
            parse_js("foo === bar", vec!["foo", "bar"], "_vm.foo === _vm.bar");
            parse_js("foo != bar", vec!["foo", "bar"], "_vm.foo != _vm.bar");
            parse_js("foo !== bar", vec!["foo", "bar"], "_vm.foo !== _vm.bar");
            parse_js("foo >= bar", vec!["foo", "bar"], "_vm.foo >= _vm.bar");
            parse_js("foo <= bar", vec!["foo", "bar"], "_vm.foo <= _vm.bar");
            parse_js(
                "foo ? foo : bar",
                vec!["foo", "foo", "bar"],
                "_vm.foo ? _vm.foo : _vm.bar",
            );
            parse_js("foo || bar", vec!["foo", "bar"], "_vm.foo || _vm.bar");
            parse_js("foo && bar", vec!["foo", "bar"], "_vm.foo && _vm.bar");

            parse_js(
                "this.foo && this.bar",
                vec!["this", "this"],
                "_vm.foo && _vm.bar",
            );
        }
    }

    mod style_tests {
        use super::*;

        fn parse_style(css: &str, expected_result: &str) {
            let mut parser = Parser::new(css);
            style::parse_scoped_css(&mut parser, style::SelectorsEnd::EOF).unwrap();

            parser = Parser::new(&format!("{}</style>", css));
            style::parse_scoped_css(&mut parser, style::SelectorsEnd::StyleClosure).unwrap();

            parser = Parser::new(&format!("{}{}", css, "}"));
            let injection_points =
                style::parse_scoped_css(&mut parser, style::SelectorsEnd::ClosingBracket).unwrap();

            let scoped_css = style::gen_scoped_css(
                &mut parser,
                &SourceLocation(0, css.len()),
                injection_points,
                "example",
            );

            assert_eq!(scoped_css, expected_result);
        }

        #[test]
        fn empty() {
            parse_style("", "");
        }

        #[test]
        fn basic_selector() {
            parse_style("foo {}", "foo[data-v-example] {}");
            parse_style("foo{}", "foo[data-v-example]{}");
        }

        #[test]
        fn invalid_selector_should_not_panic() {
            // It doesn't really matter these words are suffixed with [data-v-example] as it keeps being invalid css
            parse_style(
                "this selector is not valid as it does not contain a body",
                "this[data-v-example] selector[data-v-example] is[data-v-example] not[data-v-example] valid[data-v-example] as[data-v-example] it[data-v-example] does[data-v-example] not[data-v-example] contain[data-v-example] a[data-v-example] body",
            );
        }

        #[test]
        fn complex_selector_1() {
            parse_style("foo bar {}", "foo[data-v-example] bar[data-v-example] {}");
            parse_style(
                "foo bar baz {}",
                "foo[data-v-example] bar[data-v-example] baz[data-v-example] {}",
            );
        }

        #[test]
        fn complex_selector_2() {
            parse_style(
                "foo + bar {}",
                "foo[data-v-example] + bar[data-v-example] {}",
            );
            parse_style("foo,bar {}", "foo[data-v-example],bar[data-v-example] {}");
            parse_style("foo~bar {}", "foo[data-v-example]~bar[data-v-example] {}");
        }

        #[test]
        fn complex_selector_3() {
            parse_style("foo[arg] {}", "foo[arg][data-v-example] {}");
            parse_style("foo[arg]:hover {}", "foo[arg][data-v-example]:hover {}");
            parse_style(
                "foo[arg]:hover bar[baz]:bar_baz {}",
                "foo[arg][data-v-example]:hover bar[baz][data-v-example]:bar_baz {}",
            );
        }

        #[test]
        fn multiple_selectors() {
            parse_style(
                "
                foo {}
                bar, baz bar_foo {}
                banana + peer[with_arg] {}
                peer:hover, peer:focus {}
                ",
                "
                foo[data-v-example] {}
                bar[data-v-example], baz[data-v-example] bar_foo[data-v-example] {}
                banana[data-v-example] + peer[with_arg][data-v-example] {}
                peer[data-v-example]:hover, peer[data-v-example]:focus {}
                ",
            );
        }

        #[test]
        fn comment() {
            parse_style("/* foo { */ foo {}", "/* foo { */ foo[data-v-example] {}");
        }

        #[test]
        fn special() {
            parse_style("@charset \"UTF-8\";", "@charset \"UTF-8\";");
            parse_style(
                "@namespace svg \"http://www.w3.org/2000/svg\";",
                "@namespace svg \"http://www.w3.org/2000/svg\";",
            );
            parse_style(
                "@import 'http://example.com/style.css';",
                "@import 'http://example.com/style.css';",
            );
        }

        #[test]
        fn media() {
            parse_style(
                "@media(min-width: 1200px) {
                    h1, .h1 {font-size: 2.5rem;}
                }",
                "@media(min-width: 1200px) {
                    h1[data-v-example], .h1[data-v-example] {font-size: 2.5rem;}
                }",
            );
        }

        #[test]
        fn string() {
            parse_style(
                ".foo {
                    background-image: url(\"data:image/svg+xml,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='%23fff'%3e%3cpath d='M11.354 1.646a.5.5 0 0 1 0 .708L5.707 8l5.647 5.646a.5.5 0 0 1-.708.708l-6-6a.5.5 0 0 1 0-.708l6-6a.5.5 0 0 1 .708 0z'/%3e%3c/svg%3e\");
                }",
                ".foo[data-v-example] {
                    background-image: url(\"data:image/svg+xml,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='%23fff'%3e%3cpath d='M11.354 1.646a.5.5 0 0 1 0 .708L5.707 8l5.647 5.646a.5.5 0 0 1-.708.708l-6-6a.5.5 0 0 1 0-.708l6-6a.5.5 0 0 1 .708 0z'/%3e%3c/svg%3e\");
                }",
            );
        }

        #[test]
        fn key_frames() {
            parse_style(
                "@keyframes spinner-grow {
                    0% {
                        transform: scale(0);
                    }
                    50% {
                        opacity: 1;
                        transform: none;
                    }
                }",
                "@keyframes spinner-grow {
                    0% {
                        transform: scale(0);
                    }
                    50% {
                        opacity: 1;
                        transform: none;
                    }
                }",
            );

            parse_style(
                "@-webkit-keyframes spinner-grow {
                    0% {
                        transform: scale(0);
                    }
                    50% {
                        opacity: 1;
                        transform: none;
                    }
                }",
                "@-webkit-keyframes spinner-grow {
                    0% {
                        transform: scale(0);
                    }
                    50% {
                        opacity: 1;
                        transform: none;
                    }
                }",
            );
        }
    }
}
