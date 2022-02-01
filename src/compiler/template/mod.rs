pub mod to_js;

use super::utils::{is_space, write_str};
use super::{js, Parser, ParserError, QuoteKind, SourceLocation, TagType};

// parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
// TODO support upper case tag names
pub fn parse_tag(p: &mut Parser) -> Result<Tag, ParserError> {
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
        c = match try_parse_arg(p, c, &mut tag.args)? {
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

// Try_parse_arg parses a key="value" , :key="value" , v-bind:key="value" , v-on:key="value" and @key="value"
// It returns Ok(None) if first_char is not a char expected as first character of a argument
fn try_parse_arg(
    p: &mut Parser,
    mut c: char,
    result_args: &mut VueTagArgs,
) -> Result<Option<char>, ParserError> {
    let mut is_v_on_shotcut = false;
    let mut is_v_bind_shotcut = false;

    let mut key_location = SourceLocation(p.current_char - 1, 0);

    match c {
        '@' => {
            is_v_on_shotcut = true;
            key_location.0 += 1;
        }
        ':' => {
            is_v_bind_shotcut = true;
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
    let is_vue_arg = is_vue_dash_arg || is_v_on_shotcut || is_v_bind_shotcut;

    if is_vue_arg {
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

        if is_v_on_shotcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use @v-.. as arg name",
                ));
            }
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!(
                        "expected an argument value for \"@{}\"",
                        key_location.string(p)
                    ),
                ));
            }

            result_args.add(VueArgKind::On, key_location.string(p), value);
            return Ok(Some(c));
        }

        if is_v_bind_shotcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use :v-.. as arg name",
                ));
            }
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!(
                        "expected an argument value for \":{}\"",
                        key_location.string(p)
                    ),
                ));
            }

            result_args.add(VueArgKind::Bind, key_location.string(p), value);
            return Ok(Some(c));
        }

        // parse vue specific tag
        key_location.0 += 2;

        let vue_directives: &[(&'static str, bool, fn() -> VueArgKind)] = &[
            ("if", true, || VueArgKind::If),
            ("for", true, || VueArgKind::For),
            ("pre", true, || VueArgKind::Pre),
            ("else", false, || VueArgKind::Else),
            ("slot", true, || VueArgKind::Slot),
            ("text", true, || VueArgKind::Text),
            ("html", true, || VueArgKind::Html),
            ("show", true, || VueArgKind::Show),
            ("once", false, || VueArgKind::Once),
            ("model", true, || VueArgKind::Model),
            ("cloak", true, || VueArgKind::Cloak),
            ("else-if", true, || VueArgKind::ElseIf),
            ("bind", true, || VueArgKind::Bind),
            ("on", true, || VueArgKind::On),
        ];

        let mut vue_directives_match_input = Vec::with_capacity(vue_directives.len());
        for e in vue_directives.iter() {
            vue_directives_match_input.push(e.0.chars());
        }

        if let Some(idx) = key_location.eq_some(p, true, vue_directives_match_input) {
            let (key, expects_argument, arg_kind) = vue_directives[idx];

            if has_value != expects_argument {
                Err(ParserError::new(
                    "try_parse_arg",
                    if expects_argument {
                        format!("expected an argument value for \"v-{}\"", key)
                    } else {
                        format!("expected no argument value for \"v-{}\"", key)
                    },
                ))
            } else {
                key_location.0 += key.len();
                if p.source_chars[key_location.0] == ':' {
                    key_location.0 += 1;
                }

                result_args.add(arg_kind(), key_location.string(p), value);

                Ok(Some(c))
            }
        } else {
            key_location.0 -= 2;
            Err(ParserError::new(
                "try_parse_arg",
                format!("unknown vue argument \"{}\"", key_location.string(p)),
            ))
        }
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

        result_args.add(VueArgKind::Default, key_location.string(p), value_as_js);
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
    Var(SourceLocation, Vec<SourceLocation>),
}

impl Child {
    fn parse_children(
        p: &mut Parser,
        parents_tag_names: &mut Vec<SourceLocation>,
    ) -> Result<(Vec<Self>, SourceLocation), ParserError> {
        let mut resp: Vec<Child> = Vec::with_capacity(1);
        loop {
            let (text_node, compile_now) = Self::compile_text_node(p)?;
            if let Some(node) = text_node {
                resp.push(node);
            }

            match compile_now {
                CompileAfterTextNode::Tag => {
                    let tag = parse_tag(p)?;
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
                            let compile_children_result =
                                Self::parse_children(p, parents_tag_names);

                            let tag_name = parents_tag_names.pop().unwrap();
                            let (children, closing_tag_name) = compile_children_result?;

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
        Ok(Self::Var(
            SourceLocation(start, p.current_char - 2),
            global_vars,
        ))
    }
}

enum CompileAfterTextNode {
    Tag,
    Var,
}

#[derive(Debug, Clone)]
pub enum VueTagModifiers {
    For(String),
    If(String),
    ElseIf(String),
    Else,
}

// https://vuejs.org/v2/guide/render-function.html
// This is a somewhat rust representation of the vue component render arguments
#[derive(Debug, Clone)]
pub struct VueTagArgs {
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
    pub directives: Option<Vec<JsTagArgsDirective>>,

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

    fn add(&mut self, kind: VueArgKind, key: String, value_as_js: String) {
        self.has_js_component_args = match kind {
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
                todo!("Text");
                true
            }
            VueArgKind::Html => {
                todo!("Html");
                true
            }
            VueArgKind::Show => {
                todo!("Show");
                true
            }
            VueArgKind::If => {
                todo!("If");
                false
            }
            VueArgKind::Else => {
                todo!("Else");
                false
            }
            VueArgKind::ElseIf => {
                todo!("ElseIf");
                false
            }
            VueArgKind::For => {
                todo!("For");
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
        }
    }
}

pub enum VueArgKind {
    Default,
    Bind,
    On,
    Text,
    Html,
    Show,
    If,
    Else,
    ElseIf,
    For,
    Model,
    Slot,
    Pre,
    Cloak,
    Once,
}

#[derive(Debug, Clone)]
pub struct JsTagArgsDirective {
    pub name: String,                   // "my-custom-directive"
    pub value: String,                  // "2"
    pub expression: String,             // "1 + 1"
    pub arg: String,                    // "foo",
    pub modifiers: Vec<(String, bool)>, // { bar: true }
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
