pub mod to_js;

use super::utils::is_space;
use super::{js, Parser, ParserError, QuoteKind, SourceLocation, TagType};

// parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
// TODO support upper case tag names
pub fn parse_tag(p: &mut Parser, v_else_allowed: bool) -> Result<Tag, ParserError> {
    let mut tag = Tag {
        type_: TagType::Open,
        name: SourceLocation(p.current_char, 0),
        args: VueTagArgs::new(),
    };

    let mut is_close_tag = false;
    let mut c = p
        .seek_one()
        .ok_or(ParserError::eof("parse_tag check closure tag"))?;

    if c == '/' {
        tag.type_ = TagType::Close;
        tag.name.0 += 1;
        p.current_char += 1;
        is_close_tag = true;
    } else if c == '!' {
        p.current_char += 1;

        let mut doctype = "DOCTYPE ".chars();
        while let Some(doctype_c) = doctype.next() {
            let c = p.must_read_one()?;
            if doctype_c != c {
                return Err(ParserError::new(
                    "parse_tag",
                    format!(
                        "expected '{}' of \"<!DOCTYPE\" but got '{}'",
                        doctype_c.to_string(),
                        c.to_string()
                    ),
                ));
            }
        }

        while p.must_read_one()? != '>' {}
        return Ok(Tag {
            type_: TagType::DocType,
            name: SourceLocation::empty(),
            args: VueTagArgs::new(),
        });
    }

    // Parse names
    loop {
        match p.must_read_one()? {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => {}
            _ => {
                p.current_char -= 1;
                tag.name.1 = p.current_char;
                break;
            }
        };
    }

    if tag.name.1 == 0 {
        return Err(ParserError::new("parse_tag", "expected tag name"));
    }

    // Parse args
    loop {
        c = p.must_read_one_skip_spacing()?;
        c = match try_parse_arg(p, c, &mut tag.args, v_else_allowed)? {
            Some(next_char) => next_char,
            None => c,
        };

        match c {
            '/' => {
                if is_close_tag {
                    return Err(ParserError::new(
                        "parse_tag",
                        "/ not allowed after name in closeing tag",
                    ));
                }
                c = p.must_read_one_skip_spacing()?;
                if c != '>' {
                    return Err(ParserError::new(
                        "parse_tag",
                        format!("expected > but got '{}'", c.to_string()),
                    ));
                }
                tag.type_ = TagType::OpenAndClose;
                return Ok(tag);
            }
            '>' => return Ok(tag),
            c if is_space(c) => {}
            c => {
                return Err(ParserError::new(
                    "parse_tag",
                    format!("unexpected character '{}'", c.to_string()),
                ))
            }
        }
    }
}

fn add_or_set<T>(list: &mut Option<Vec<T>>, add: T) {
    if let Some(list) = list.as_mut() {
        list.push(add);
    } else {
        *list = Some(vec![add]);
    }
}

#[derive(Debug, Clone)]
struct ParsedVFor {
    value: String,
    key: Option<String>,
    index: Option<String>,
    list: String,
}

fn parse_v_for_value(p: &mut Parser) -> Result<ParsedVFor, ParserError> {
    let closure = p.must_read_one()?;
    match closure {
        '"' | '\'' => {} // Ok
        c => {
            return Err(ParserError::new(
                "parse_v_for_value",
                format!(
                    "expected opening of argument value ('\"' or \"'\") but got '{}'",
                    c.to_string()
                ),
            ))
        }
    }

    // if true `foo in bar`, if false: `(foo, idx) in bar` or `(foo) in bar`
    let mut is_single = false;

    // Look for the start of the name
    // `(foo) in bar`
    //   ^- Find this
    match p.must_read_one_skip_spacing()? {
        '(' => {
            let c = p.must_read_one_skip_spacing()?;
            if !c.is_ascii_lowercase() && !c.is_ascii_uppercase() && c <= '}' {
                return Err(ParserError::new(
                    "parse_v_for_value",
                    format!("unexpected character '{}'", c.to_string()),
                ));
            }
        }
        c if c.is_ascii_lowercase() || c.is_ascii_uppercase() || c > '}' => {
            is_single = true;
        }
        c => {
            return Err(ParserError::new(
                "parse_v_for_value",
                format!("unexpected character '{}'", c.to_string()),
            ))
        }
    }

    let (mut c, value_location) = js::parse_name(p)?;
    if is_space(c) {
        c = p.must_read_one_skip_spacing()?;
    }

    let mut result = ParsedVFor {
        value: value_location.string(p),
        key: None,
        index: None,
        list: String::new(),
    };

    if !is_single {
        if c == ',' {
            // Read the key
            // `v-for"(value, key) in list"`
            //                ^- That one
            p.must_read_one_skip_spacing()?;

            let (next_c, key_location) = js::parse_name(p)?;
            c = next_c;
            result.key = Some(key_location.string(p));

            if is_space(c) {
                c = p.must_read_one_skip_spacing()?;
            }

            if c == ',' {
                // Read the index
                // `v-for"(value, key, index) in object"`
                //                     ^- That one
                p.must_read_one_skip_spacing()?;

                let (next_c, index_location) = js::parse_name(p)?;
                c = next_c;
                result.index = Some(index_location.string(p));

                if is_space(c) {
                    c = p.must_read_one_skip_spacing()?;
                }
            }
        }

        if c != ')' {
            return Err(ParserError::new(
                "parse_v_for_value",
                format!("expected ')' but got '{}'", c.to_string()),
            ));
        }
        c = p.must_read_one_skip_spacing()?;
    }

    if c != 'i' {
        return Err(ParserError::new(
            "parse_v_for_value",
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }
    c = p.must_read_one()?;
    if c != 'n' {
        return Err(ParserError::new(
            "parse_v_for_value",
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }
    c = p.must_read_one()?;
    if !is_space(c) {
        return Err(ParserError::new(
            "parse_v_for_value",
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }

    let start = p.current_char;
    let replacements = js::parse_template_arg(p, closure)?;
    let list_location = SourceLocation(start, p.current_char - 1);
    result.list = js::add_vm_references(p, &list_location, &replacements);

    Ok(result)
}

// Try_parse_arg parses a key="value" , :key="value" , v-bind:key="value" , v-on:key="value" and @key="value"
// It returns Ok(None) if first_char is not a char expected as first character of a argument
fn try_parse_arg(
    p: &mut Parser,
    mut c: char,
    result_args: &mut VueTagArgs,
    v_else_allowed: bool,
) -> Result<Option<char>, ParserError> {
    let mut is_v_on_shortcut = false;
    let mut is_v_bind_shortcut = false;

    let mut key_location = SourceLocation(p.current_char - 1, 0);

    match c {
        '@' => {
            is_v_on_shortcut = true;
            key_location.0 += 1;
        }
        ':' => {
            is_v_bind_shortcut = true;
            key_location.0 += 1;
        }
        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {}
        _ => return Ok(None),
    };

    let mut has_value = false;
    loop {
        c = p.must_read_one()?;
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' => {}
            '=' => {
                let c = p.must_seek_one()?;
                has_value = !is_space(c) && c != '/' && c != '>';
                break;
            }
            c if is_space(c) || c == '/' || c == '>' => {
                break;
            }
            c => {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!("unexpected argument character '{}'", c.to_string()),
                ))
            }
        }
    }
    key_location.1 = p.current_char - 1;
    let is_vue_dash_arg = key_location.starts_with(p, "v-".chars());
    let is_vue_arg = is_vue_dash_arg || is_v_on_shortcut || is_v_bind_shortcut;
    let mut key = key_location.string(p);

    if is_vue_arg {
        // parse vue specific tag
        if key == "v-for" {
            // Parse the value of the v-for tag
            // V-for has a special value we cannot parse like the others
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "expected an argument value for \"v-for\"",
                ));
            }

            let result = parse_v_for_value(p)?;

            let mut local_variables_list: Vec<String> = vec![result.value.clone()];
            if let Some(key) = result.key.as_ref() {
                local_variables_list.push(key.clone());
                if let Some(index) = result.index.as_ref() {
                    local_variables_list.push(index.clone());
                }
            }
            result_args.new_local_variables = Some(local_variables_list);
            result_args.set_modifier(VueTagModifier::For(result))?;

            c = p.must_read_one()?;
            return Ok(Some(c));
        }

        let value: String = if has_value {
            let closure = p.must_read_one()?;
            match closure {
                '"' | '\'' => {} // Ok
                c => {
                    return Err(ParserError::new(
                        "try_parse_arg",
                        format!(
                            "expected opening of argument value ('\"' or \"'\") but got '{}'",
                            c.to_string()
                        ),
                    ))
                }
            }
            let start = p.current_char;
            let replacements = js::parse_template_arg(p, closure)?;
            let sl = SourceLocation(start, p.current_char - 1);
            c = p.must_read_one()?;

            js::add_vm_references(p, &sl, &replacements)
        } else {
            String::from("undefined")
        };

        if is_v_on_shortcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use @v-.. as arg name",
                ));
            }
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!("expected an argument value for \"@{}\"", key),
                ));
            }

            result_args.add(VueArgKind::On, key, value)?;
            return Ok(Some(c));
        }

        if is_v_bind_shortcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use :v-.. as arg name",
                ));
            }
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!("expected an argument value for \":{}\"", key),
                ));
            }

            result_args.add(VueArgKind::Bind, key, value)?;
            return Ok(Some(c));
        }

        // remove the v- from the argument
        key.replace_range(..2, "");

        let (expects_argument, arg_kind) = match key.as_str() {
            "if" => (true, VueArgKind::If),
            "pre" => (true, VueArgKind::Pre),
            "else" => (false, VueArgKind::Else),
            "slot" => (true, VueArgKind::Slot),
            key if key.starts_with("slot") => (true, VueArgKind::Slot),
            "text" => (true, VueArgKind::Text),
            "html" => (true, VueArgKind::Html),
            "once" => (false, VueArgKind::Once),
            "model" => (true, VueArgKind::Model),
            key if key.starts_with("model") => (true, VueArgKind::Model),
            "cloak" => (true, VueArgKind::Cloak),
            "else-if" => (true, VueArgKind::ElseIf),
            "bind" => (true, VueArgKind::Bind),
            key if key.starts_with("bind:") => (true, VueArgKind::Bind),
            "on" => (true, VueArgKind::On),
            key if key.starts_with("on") => (true, VueArgKind::On),
            _ => (true, VueArgKind::CustomDirective(key.clone())),
        };

        if has_value != expects_argument {
            return Err(ParserError::new(
                "try_parse_arg",
                if expects_argument {
                    format!("expected an argument value for \"v-{}\"", key)
                } else {
                    format!("expected no argument value for \"v-{}\"", key)
                },
            ));
        }

        key = match key.split_once(':') {
            Some((_, after)) => after.to_string(),
            None => String::new(),
        };

        if !v_else_allowed {
            match arg_kind {
                VueArgKind::ElseIf => {
                    return Err(ParserError::new(
                        "try_parse_arg",
                        "cannot use v-else-if here",
                    ));
                }
                VueArgKind::Else => {
                    return Err(ParserError::new("try_parse_arg", "cannot use v-else here"));
                }
                _ => {}
            }
        }

        result_args.add(arg_kind, key, value)?;
        Ok(Some(c))
    } else {
        let value_as_js: String = if has_value {
            // Parse a static argument
            let value_location = match p.must_read_one()? {
                '"' => {
                    let start = p.current_char;
                    p.parse_quotes(QuoteKind::HTMLDouble, &mut None)?;
                    let sl = SourceLocation(start, p.current_char - 1);
                    c = p.must_read_one()?;
                    sl
                }
                '\'' => {
                    let start = p.current_char;
                    p.parse_quotes(QuoteKind::HTMLSingle, &mut None)?;
                    let sl = SourceLocation(start, p.current_char - 1);
                    c = p.must_read_one()?;
                    sl
                }
                _ => {
                    let start = p.current_char - 1;
                    loop {
                        c = p.must_read_one()?;
                        match c {
                            '>' | '/' => {
                                break;
                            }
                            c if is_space(c) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    SourceLocation(start, p.current_char - 1)
                }
            };

            let mut s = String::new();
            s.push('"');
            for c in value_location.chars(p) {
                match c {
                    '\\' => {
                        s.push('\\');
                        s.push('\\');
                    }
                    '"' => {
                        s.push('\\');
                        s.push('"');
                    }
                    c => s.push(*c),
                }
            }
            s.push('"');
            s
        } else {
            String::from("true")
        };

        result_args.add(VueArgKind::Default, key_location.string(p), value_as_js)?;
        Ok(Some(c))
    }
}

pub fn compile(p: &mut Parser) -> Result<Vec<Child>, ParserError> {
    let mut compile_result = Child::parse_children(p, &mut Vec::new())?;
    loop {
        if compile_result.1.eq(p, "template".chars()) {
            return Ok(compile_result.0);
        } else {
            compile_result = Child::parse_children(p, &mut Vec::new())?;
        }
    }
}

#[derive(Debug, Clone)]
pub enum Child {
    Tag(Tag, Vec<Child>),
    Text(SourceLocation),
    Var(String),
}

impl Child {
    fn parse_children(
        p: &mut Parser,
        parents_tag_names: &mut Vec<SourceLocation>,
    ) -> Result<(Vec<Self>, SourceLocation), ParserError> {
        let mut resp: Vec<Child> = Vec::with_capacity(1);
        let mut inside_v_if = false;
        loop {
            let (text_node, compile_now) = Self::compile_text_node(p)?;
            if let Some(node) = text_node {
                inside_v_if = false;
                resp.push(node);
            }

            match compile_now {
                CompileAfterTextNode::Tag => {
                    let tag = parse_tag(p, inside_v_if)?;

                    if let Some(modifier) = tag.args.modifier.as_ref() {
                        inside_v_if = match modifier {
                            VueTagModifier::If(_) => true,
                            VueTagModifier::ElseIf(_) => true,
                            _ => false,
                        };
                    } else {
                        inside_v_if = false;
                    }

                    match tag.type_ {
                        TagType::Close => {
                            let mut found = false;
                            for parent in parents_tag_names.iter().rev() {
                                if tag.name.eq_self(p, parent) {
                                    found = true;
                                    break;
                                }
                            }
                            if found || tag.name.eq(p, "template".chars()) {
                                return Ok((resp, tag.name));
                            }
                        }
                        TagType::Open => {
                            parents_tag_names.push(tag.name.clone());

                            let local_variables = tag.args.new_local_variables.as_ref();

                            // Add the new local variables if there where some
                            if let Some(new_local_variables) = local_variables {
                                for var_name in new_local_variables {
                                    if let Some(count) = p.local_variables.get_mut(var_name) {
                                        *count += 1;
                                    } else {
                                        p.local_variables.insert(var_name.clone(), 1);
                                    }
                                }
                            }

                            let compile_children_result =
                                Self::parse_children(p, parents_tag_names);

                            let tag_name = parents_tag_names.pop().unwrap();
                            let (children, closing_tag_name) = compile_children_result?;

                            // Remove the local variables we above inserted
                            if let Some(new_local_variables) = local_variables {
                                for var_name in new_local_variables {
                                    if let Some(count) = p.local_variables.get_mut(var_name) {
                                        if *count == 1 {
                                            p.local_variables.remove(var_name);
                                        } else {
                                            *count -= 1;
                                        }
                                    }
                                }
                            }

                            resp.push(Self::Tag(tag, children));

                            let correct_closing_tag = tag_name.eq_self(p, &closing_tag_name);
                            if !correct_closing_tag {
                                return Ok((resp, closing_tag_name));
                            }
                        }
                        TagType::OpenAndClose => {
                            resp.push(Self::Tag(tag, Vec::new()));
                        }
                        TagType::DocType => {} // Skip this tag
                    };
                }
                CompileAfterTextNode::Var => {
                    inside_v_if = false;
                    resp.push(Self::parse_var(p)?);
                }
            }
        }
    }

    fn compile_text_node(
        p: &mut Parser,
    ) -> Result<(Option<Self>, CompileAfterTextNode), ParserError> {
        let text_node_start = p.current_char;
        let mut only_spaces = true;

        let gen_resp = |p: &mut Parser, only_spaces: bool| {
            if only_spaces {
                // We do not care about strings with only spaces
                None
            } else {
                let resp = SourceLocation(text_node_start, p.current_char - 1);
                if resp.is_empty() {
                    None
                } else {
                    Some(Self::Text(resp))
                }
            }
        };

        loop {
            match p.must_read_one()? {
                '<' => return Ok((gen_resp(p, only_spaces), CompileAfterTextNode::Tag)),
                '{' => {
                    if let Some(c) = p.seek_one() {
                        if c == '{' {
                            let resp = gen_resp(p, only_spaces);
                            p.current_char += 1;
                            return Ok((resp, CompileAfterTextNode::Var));
                        }
                    }
                }
                c if only_spaces && is_space(c) => {}
                _ => only_spaces = false,
            }
        }
    }

    fn parse_var(p: &mut Parser) -> Result<Self, ParserError> {
        let start = p.current_char;
        let global_vars = js::parse_template_var(p)?;
        let var = SourceLocation(start, p.current_char - 2);
        Ok(Self::Var(js::add_vm_references(p, &var, &global_vars)))
    }

    pub fn is_v_else_or_else_if(&self) -> bool {
        if let Child::Tag(tag, _) = self {
            if let Some(modifier) = tag.args.modifier.as_ref() {
                match modifier {
                    VueTagModifier::ElseIf(_) | VueTagModifier::Else => return true,
                    _ => {}
                }
            }
        }
        false
    }

    pub fn is_v_for(&self) -> bool {
        if let Child::Tag(tag, _) = self {
            if let Some(modifier) = tag.args.modifier.as_ref() {
                if let VueTagModifier::For(_) = modifier {
                    return true;
                }
            }
        }
        false
    }
}

enum CompileAfterTextNode {
    Tag,
    Var,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub type_: TagType,
    pub name: SourceLocation,
    pub args: VueTagArgs,
}

impl Tag {
    pub fn is_custom_component(&self, parser: &Parser) -> bool {
        let html_elements = vec![
            "a".chars(),
            "abbr".chars(),
            "acronym".chars(),
            "address".chars(),
            "applet".chars(),
            "area".chars(),
            "article".chars(),
            "aside".chars(),
            "audio".chars(),
            "b".chars(),
            "base".chars(),
            "basefont".chars(),
            "bdi".chars(),
            "bdo".chars(),
            "big".chars(),
            "blockquote".chars(),
            "body".chars(),
            "br".chars(),
            "button".chars(),
            "canvas".chars(),
            "caption".chars(),
            "center".chars(),
            "cite".chars(),
            "code".chars(),
            "col".chars(),
            "colgroup".chars(),
            "data".chars(),
            "datalist".chars(),
            "dd".chars(),
            "del".chars(),
            "details".chars(),
            "dfn".chars(),
            "dialog".chars(),
            "dir".chars(),
            "div".chars(),
            "dl".chars(),
            "dt".chars(),
            "em".chars(),
            "embed".chars(),
            "fieldset".chars(),
            "figcaption".chars(),
            "figure".chars(),
            "font".chars(),
            "footer".chars(),
            "form".chars(),
            "frame".chars(),
            "frameset".chars(),
            "head".chars(),
            "header".chars(),
            "hgroup".chars(),
            "h1".chars(),
            "h2".chars(),
            "h3".chars(),
            "h4".chars(),
            "h5".chars(),
            "h6".chars(),
            "hr".chars(),
            "html".chars(),
            "i".chars(),
            "iframe".chars(),
            "img".chars(),
            "input".chars(),
            "ins".chars(),
            "kbd".chars(),
            "keygen".chars(),
            "label".chars(),
            "legend".chars(),
            "li".chars(),
            "link".chars(),
            "main".chars(),
            "map".chars(),
            "mark".chars(),
            "menu".chars(),
            "menuitem".chars(),
            "meta".chars(),
            "meter".chars(),
            "nav".chars(),
            "noframes".chars(),
            "noscript".chars(),
            "object".chars(),
            "ol".chars(),
            "optgroup".chars(),
            "option".chars(),
            "output".chars(),
            "p".chars(),
            "param".chars(),
            "picture".chars(),
            "pre".chars(),
            "progress".chars(),
            "q".chars(),
            "rp".chars(),
            "rt".chars(),
            "ruby".chars(),
            "s".chars(),
            "samp".chars(),
            "script".chars(),
            "section".chars(),
            "select".chars(),
            "small".chars(),
            "source".chars(),
            "span".chars(),
            "strike".chars(),
            "strong".chars(),
            "style".chars(),
            "sub".chars(),
            "summary".chars(),
            "sup".chars(),
            "svg".chars(),
            "table".chars(),
            "tbody".chars(),
            "td".chars(),
            "template".chars(),
            "textarea".chars(),
            "tfoot".chars(),
            "th".chars(),
            "thead".chars(),
            "time".chars(),
            "title".chars(),
            "tr".chars(),
            "track".chars(),
            "tt".chars(),
            "u".chars(),
            "ul".chars(),
            "var".chars(),
            "video".chars(),
            "wbr".chars(),
        ];

        self.name.eq_some(parser, false, html_elements).is_none()
    }
}

// https://vuejs.org/v2/guide/render-function.html
// This is a somewhat rust representation of the vue component render arguments
#[derive(Debug, Clone)]
pub struct VueTagArgs {
    pub new_local_variables: Option<Vec<String>>,
    pub modifier: Option<VueTagModifier>,
    pub has_js_component_args: bool,

    // Same API as `v-bind:class`, accepting either
    // a string, object, or array of strings and objects.
    // {foo: true, bar: false}
    pub class: Option<String>,

    // Same API as `v-bind:style`, accepting either
    // a string, object, or array of objects.
    //{ color: 'red', fontSize: '14px'}
    pub style: Option<String>,

    // Normal HTML attributes
    // OR
    // Component props
    // { foo: 'bar' }
    pub attrs_or_props: Option<Vec<(String, String)>>,

    // DOM properties
    // domProps: { innerHTML: 'baz' }
    pub dom_props: Option<Vec<(String, String)>>,

    // Event handlers are nested under `on`, though
    // modifiers such as in `v-on:keyup.enter` are not
    // supported. You'll have to manually check the
    // keyCode in the handler instead.
    // { click: this.clickHandler }
    pub on: Option<Vec<(String, String)>>,

    // For components only. Allows you to listen to
    // native events, rather than events emitted from
    // the component using `vm.$emit`.
    // nativeOn: { click: this.nativeClickHandler }
    pub native_on: Option<Vec<(String, String)>>,

    // Custom directives. Note that the `binding`'s
    // `oldValue` cannot be set, as Vue keeps track
    // of it for you.
    pub directives: Option<Vec<(String, String)>>,

    // TODO
    // Scoped slots in the form of
    // { name: props => VNode | Array<VNode> }
    // scopedSlots: {
    //   default: props => createElement('span', props.text)
    // },

    // The name of the slot, if this component is the
    // child of another component
    pub slot: Option<String>, // "name-of-slot"

    // Other special top-level properties
    // "myKey"
    pub key: Option<String>,
    // ref = "myRef"
    pub ref_: Option<String>,

    // If you are applying the same ref name to multiple
    // elements in the render function. This will make `$refs.myRef` become an array
    // refInFor = true
    pub ref_in_for: Option<bool>,
}

impl VueTagArgs {
    fn new() -> Self {
        Self {
            new_local_variables: None,
            modifier: None,
            has_js_component_args: false,
            class: None,
            style: None,
            attrs_or_props: None,
            dom_props: None,
            on: None,
            native_on: None,
            directives: None,
            slot: None,
            key: None,
            ref_: None,
            ref_in_for: None,
        }
    }

    pub fn has_attr_or_prop(&self, name: &str) -> Option<&str> {
        if let Some(attrs_or_props) = self.attrs_or_props.as_ref() {
            for (key, js_value) in attrs_or_props {
                if key == name {
                    return Some(&js_value);
                }
            }
        }
        None
    }

    pub fn has_attr_or_prop_with_string(&self, name: &str) -> Option<String> {
        let mut value = self.has_attr_or_prop(name)?.chars();

        let quote = match value.next()? {
            '\'' => '\'',
            '"' => '"',
            '`' => '`',
            _ => return None,
        };

        let mut resp = String::new();
        loop {
            match value.next()? {
                c if c == quote => break,
                '\\' => resp.push(value.next()?),
                c => resp.push(c),
            }
        }

        Some(resp)
    }

    fn add(
        &mut self,
        kind: VueArgKind,
        key: String,
        value_as_js: String,
    ) -> Result<(), ParserError> {
        let set_has_js_component_args = match kind {
            VueArgKind::Default | VueArgKind::Bind => {
                match key.as_str() {
                    "class" => self.class = Some(value_as_js),
                    "style" => self.style = Some(value_as_js),
                    "slot" => self.slot = Some(value_as_js),
                    "key" => self.key = Some(value_as_js),
                    "ref" => self.ref_ = Some(value_as_js),
                    _ => add_or_set(&mut self.attrs_or_props, (key, value_as_js)),
                };
                true
            }
            VueArgKind::On => {
                add_or_set(&mut self.on, (key, value_as_js));
                true
            }
            VueArgKind::Text => {
                add_or_set(
                    &mut self.dom_props,
                    (String::from("textContent"), value_as_js),
                );
                true
            }
            VueArgKind::Html => {
                add_or_set(
                    &mut self.dom_props,
                    (String::from("innerHTML"), value_as_js),
                );
                true
            }
            VueArgKind::If => {
                self.set_modifier(VueTagModifier::If(value_as_js))?;
                false
            }
            VueArgKind::Else => {
                self.set_modifier(VueTagModifier::Else)?;
                false
            }
            VueArgKind::ElseIf => {
                self.set_modifier(VueTagModifier::ElseIf(value_as_js))?;
                false
            }
            VueArgKind::Model => {
                todo!("Model");
                true
            }
            VueArgKind::Slot => {
                todo!("Slot");
                true
            }
            VueArgKind::Pre => {
                todo!("Pre");
                true
            }
            VueArgKind::Cloak => {
                todo!("Cloak");
                true
            }
            VueArgKind::Once => {
                todo!("Once");
                true
            }
            VueArgKind::CustomDirective(directive) => {
                add_or_set(&mut self.directives, (directive, value_as_js));
                true
            }
        };
        if set_has_js_component_args {
            self.has_js_component_args = true;
        }
        Ok(())
    }
    fn set_modifier(&mut self, to: VueTagModifier) -> Result<(), ParserError> {
        if let Some(already_set_modifier) = self.modifier.as_ref() {
            Err(ParserError::new(
                "VueTagArgs::add",
                format!(
                    "cannot set {} on a tag that also has {}",
                    to.kind(),
                    already_set_modifier.kind()
                ),
            ))
        } else {
            self.modifier = Some(to);
            Ok(())
        }
    }
}

pub enum VueArgKind {
    Default,
    Bind,
    On,
    Text,
    Html,
    If,
    Else,
    ElseIf,
    // For, // This one is handled specially
    Model,
    Slot,
    Pre,
    Cloak,
    Once,
    CustomDirective(String),
}

#[derive(Debug, Clone)]
pub enum VueTagModifier {
    For(ParsedVFor),
    If(String),
    ElseIf(String),
    Else,
}

impl VueTagModifier {
    fn kind(&self) -> &'static str {
        match self {
            Self::For(_) => "v-for",
            Self::If(_) => "v-if",
            Self::ElseIf(_) => "v-else-if",
            Self::Else => "v-else",
        }
    }
}
