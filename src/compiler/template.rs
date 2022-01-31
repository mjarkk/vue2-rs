use super::utils::{is_space, write_str};
use super::{js, Parser, ParserError, QuoteKind, SourceLocation, TagType};

// parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
// TODO support upper case tag names
pub fn parse_tag(p: &mut Parser) -> Result<Tag, ParserError> {
    let mut tag = Tag {
        type_: TagType::Open,
        name: SourceLocation(p.current_char, 0),
        args: Vec::new(),
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
            args: vec![],
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
        c = match try_parse_arg(p, c)? {
            Some((arg, next_char)) => {
                tag.args.push(arg);
                next_char
            }
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

// Try_parse_arg parses a key="value" , :key="value" , v-bind:key="value" , v-on:key="value" and @key="value"
// It returns Ok(None) if first_char is not a char expected as first character of a argument
fn try_parse_arg(
    p: &mut Parser,
    mut c: char,
    result_args: &mut JsTagArgs,
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
        let value_location: Option<(SourceLocation, Vec<SourceLocation>)> = if has_value {
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
            Some((sl, replacements))
        } else {
            None
        };

        if is_v_on_shotcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use @v-.. as arg name",
                ));
            }
            return if let Some(value_location) = value_location {
                let kv = (
                    key_location,
                    js::add_vm_references(p, &value_location.0, &value_location.1),
                );
                if let Some(list) = result_args.on.as_ref() {
                    list.push(kv);
                } else {
                    result_args.on = Some(vec![kv]);
                }

                Ok(Some(c))
            } else {
                Err(ParserError::new(
                    "try_parse_arg",
                    format!(
                        "expected an argument value for \"@{}\"",
                        key_location.string(p)
                    ),
                ))
            };
        } else if is_v_bind_shotcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use :v-.. as arg name",
                ));
            }
            return if let Some(value_location) = value_location {
                Ok(Some((TagArg::Bind(key_location, value_location), c)))
            } else {
                Err(ParserError::new(
                    "try_parse_arg",
                    format!(
                        "expected an argument value for \":{}\"",
                        key_location.string(p)
                    ),
                ))
            };
        }

        // parse vue spesific tag
        key_location.0 += 2;

        let vue_directives: &[(
            &'static str,
            bool,
            fn(k: SourceLocation, v: (SourceLocation, Vec<SourceLocation>)) -> TagArg,
        )] = &[
            ("if", true, |_, v| TagArg::If(v)),
            ("for", true, |_, v| TagArg::For(v)),
            ("pre", true, |_, v| TagArg::Pre(v)),
            ("else", false, |_, _| TagArg::Else),
            ("slot", true, |_, v| TagArg::Slot(v)),
            ("text", true, |_, v| TagArg::Text(v)),
            ("html", true, |_, v| TagArg::Html(v)),
            ("show", true, |_, v| TagArg::Show(v)),
            ("once", false, |_, _| TagArg::Once),
            ("model", true, |_, v| TagArg::Model(v)),
            ("cloak", true, |_, v| TagArg::Cloak(v)),
            ("else-if", true, |_, v| TagArg::ElseIf(v)),
            ("bind", true, |k, v| TagArg::Bind(k, v)),
            ("on", true, |k, v| TagArg::On(k, v)),
        ];

        let mut vue_directives_match_input = Vec::with_capacity(vue_directives.len());
        for e in vue_directives.iter() {
            vue_directives_match_input.push(e.0.chars());
        }

        if let Some(idx) = key_location.eq_some(p, true, vue_directives_match_input) {
            let (key, expects_argument, make_result_tag) = vue_directives[idx];

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

                let tag = make_result_tag(
                    key_location,
                    value_location.unwrap_or((SourceLocation::empty(), Vec::new())),
                );
                Ok(Some((tag, c)))
            }
        } else {
            key_location.0 -= 2;
            Err(ParserError::new(
                "try_parse_arg",
                format!("unknown vue argument \"{}\"", key_location.string(p)),
            ))
        }
    } else {
        let value: String = if has_value {
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

        if let Some(list) = result_args.attrs_or_props.as_mut() {
            list.push((key_location, value));
        } else {
            result_args.attrs_or_props = Some(vec![(key_location, value)])
        }

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

    pub fn to_js(&self, p: &Parser, resp: &mut Vec<char>) {
        // TODO support html escape
        match self {
            Self::Tag(tag, children) => {
                // Writes:
                // _c('div', [_c(..), _c(..)])
                write_str("_c('", resp);
                tag.name.write_to_vec_escape(p, resp, '\'', '\\');
                resp.push('\'');
                if tag.args.len() != 0 {
                    let is_custom_component = tag.is_custom_component(p);

                    let mut js_tag_args = JsTagArgs::new();
                    for arg in tag.args.iter() {
                        arg.insert_into_js_tag_args(p, &mut js_tag_args, is_custom_component);
                    }

                    resp.push(',');
                    js_tag_args.to_js(p, resp);
                }

                write_str(",[", resp);
                let children_len = children.len();
                if children_len != 0 {
                    let children_max_idx = children_len - 1;
                    for (idx, child) in children.iter().enumerate() {
                        child.to_js(p, resp);
                        if idx != children_max_idx {
                            resp.push(',');
                        }
                    }
                }
                write_str("])", resp);
            }
            Self::Text(location) => {
                // Writes:
                // _vm._v("foo bar")
                write_str("_vm._v(\"", resp);
                for c in location.chars(p).iter() {
                    match *c {
                        '\\' | '"' => {
                            // Add escape characters
                            resp.push('\\');
                            resp.push(*c);
                        }
                        c => resp.push(c),
                    }
                }
                write_str("\")", resp);
            }
            Self::Var(var, global_refs) => {
                write_str("_vm._s(", resp);
                write_str(&js::add_vm_references(p, var, global_refs), resp);
                resp.push(')');
            }
        }
    }
}

enum CompileAfterTextNode {
    Tag,
    Var,
}

const DEFAULT_CONF: &'static str = "
__vue_2_file_default_export__.render = c => {
    const _vm = this;
    const _h = _vm.$createElement;
    const _c = _vm._self._c || _h;
    return ";

/*
_c('div', [
    _c('h1', [
        _vm._v(\"It wurks \" + _vm._s(_vm.count) + \" !\")
    ]),
    _c('button', { on: { \"click\": $event => { _vm.count++ } } }, [_vm._v(\"+\")]),
    _c('button', { on: { \"click\": $event => { _vm.count-- } } }, [_vm._v(\"-\")]),
])
*/

pub fn convert_template_to_js_render_fn(p: &Parser, resp: &mut Vec<char>) {
    let template = match p.template.as_ref() {
        Some(t) => t,
        None => return,
    };

    resp.append(&mut DEFAULT_CONF.chars().collect());

    match template.content.len() {
        0 => {
            write_str("[]", resp);
        }
        1 => {
            template.content.get(0).unwrap().to_js(p, resp);
        }
        content_len => {
            resp.push('[');
            for (idx, child) in template.content.iter().enumerate() {
                child.to_js(p, resp);
                if idx + 1 != content_len {
                    resp.push(',');
                }
            }
            resp.push(']');
        }
    }

    resp.append(&mut "\n};".chars().collect())
}

// https://vuejs.org/v2/guide/render-function.html
// This is a somewhat rust representation of the vue component render arguments
#[derive(Debug)]
pub struct JsTagArgs {
    // Same API as `v-bind:class`, accepting either
    // a string, object, or array of strings and objects.
    // {foo: true, bar: false}
    pub class: Option<String>,

    // Same API as `v-bind:style`, accepting either
    // a string, object, or array of objects.
    //{ color: 'red', fontSize: '14px'}
    pub style: Option<String>,

    // Normal HTML attributes
    // { foo: 'bar' }
    pub attrs: Option<Vec<(SourceLocation, String)>>,

    // Component props
    // { myProp: 'bar' }
    pub props: Option<Vec<(SourceLocation, String)>>,

    pub attrs_or_props: Option<Vec<(SourceLocation, String)>>,

    // DOM properties
    // domProps: { innerHTML: 'baz' }
    pub dom_props: Option<Vec<(SourceLocation, String)>>,

    // Event handlers are nested under `on`, though
    // modifiers such as in `v-on:keyup.enter` are not
    // supported. You'll have to manually check the
    // keyCode in the handler instead.
    // { click: this.clickHandler }
    pub on: Option<Vec<(SourceLocation, String)>>,

    // For components only. Allows you to listen to
    // native events, rather than events emitted from
    // the component using `vm.$emit`.
    // nativeOn: { click: this.nativeClickHandler }
    pub native_on: Option<Vec<(SourceLocation, String)>>,

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

impl JsTagArgs {
    fn new() -> Self {
        Self {
            class: None,
            style: None,
            attrs: None,
            props: None,
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

    fn to_js(&self, p: &Parser, dest: &mut Vec<char>) {
        dest.push('{');
        let mut object_entries = CommaSeperatedEntries::new();

        // TODO: class // Option<SourceLocation>,
        // TODO: style // Option<SourceLocation>,

        if let Some(attrs) = self.attrs.as_ref() {
            object_entries.add(dest);
            write_str("attrs:{", dest);
            let mut attrs_entries = CommaSeperatedEntries::new();

            for (key, value) in attrs {
                attrs_entries.add(dest);

                dest.push('"');
                key.write_to_vec_escape(p, dest, '"', '\\');
                write_str("\":", dest);

                for c in value.chars() {
                    dest.push(c);
                }
            }

            dest.push('}');
        }

        if let Some(props) = self.props.as_ref() {
            object_entries.add(dest);
            write_str("props:{", dest);
            let mut props_entries = CommaSeperatedEntries::new();

            for (key, value) in props {
                props_entries.add(dest);

                dest.push('"');
                key.write_to_vec_escape(p, dest, '"', '\\');
                write_str("\":", dest);

                for c in value.chars() {
                    dest.push(c);
                }
            }

            dest.push('}');
        }

        // TODO: dom_props // Option<Vec<(SourceLocation, SourceLocation)>>,

        if let Some(on) = self.on.as_ref() {
            object_entries.add(dest);
            write_str("on:{", dest);
            let mut on_entries = CommaSeperatedEntries::new();

            for (key, value) in on {
                on_entries.add(dest);

                dest.push('"');
                key.write_to_vec_escape(p, dest, '"', '\\');
                write_str("\":$event=>{", dest);

                for c in value.chars() {
                    dest.push(c);
                }

                dest.push('}');
            }

            dest.push('}');
        }

        // TODO: native_on // Option<Vec<(SourceLocation, SourceLocation)>>,
        // TODO: directives // Option<Vec<JsTagArgsDirective>>,
        // TODO: slot // Option<String>, // "name-of-slot"
        // TODO: key // Option<String>,
        // TODO: ref_ // Option<String>,
        // TODO: ref_in_for // Option<bool>,
        dest.push('}');
    }
}

struct CommaSeperatedEntries(bool);

impl CommaSeperatedEntries {
    fn new() -> Self {
        Self(false)
    }
    fn add(&mut self, dest: &mut Vec<char>) {
        if self.0 {
            dest.push(',');
        } else {
            self.0 = true;
        }
    }
}

#[derive(Debug)]
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
    pub args: Vec<TagArg>,
}

impl Tag {
    pub fn arg(&self, parser: &Parser, key: &str) -> Option<&TagArg> {
        for arg in self.args.iter() {
            if arg.key_eq(parser, key) {
                return Some(arg);
            }
        }
        None
    }
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

#[derive(Debug, Clone)]
pub enum TagArg {
    Default(SourceLocation, Option<SourceLocation>), // value="val"
    Bind(SourceLocation, (SourceLocation, Vec<SourceLocation>)), // :value="val" and v-bind:value="val"
    On(SourceLocation, (SourceLocation, Vec<SourceLocation>)),   // @click and v-on:click="val"
    Text((SourceLocation, Vec<SourceLocation>)),                 // v-text=""
    Html((SourceLocation, Vec<SourceLocation>)),                 // v-html=""
    Show((SourceLocation, Vec<SourceLocation>)),                 // v-show=""
    If((SourceLocation, Vec<SourceLocation>)),                   // v-if=""
    Else,                                                        // v-else
    ElseIf((SourceLocation, Vec<SourceLocation>)),               // v-else-if
    For((SourceLocation, Vec<SourceLocation>)),                  // v-for=""
    Model((SourceLocation, Vec<SourceLocation>)),                // v-model=""
    Slot((SourceLocation, Vec<SourceLocation>)),                 // v-slot=""
    Pre((SourceLocation, Vec<SourceLocation>)),                  // v-pre=""
    Cloak((SourceLocation, Vec<SourceLocation>)),                // v-cloak=""
    Once,                                                        // v-once
}

impl TagArg {
    pub fn insert_into_js_tag_args(
        &self,
        p: &Parser,
        add_to: &mut JsTagArgs,
        is_custom_component: bool,
    ) {
        let todo = |v| todo!("support {}", v);

        match self {
            Self::Default(key, value) => {
                let js_value = match value {
                    Some(v) => {
                        let mut s = String::new();
                        s.push('"');
                        for c in v.chars(p) {
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
                    }
                    None => String::from("true"),
                };
                let kv = (key.clone(), js_value);

                let add_to_list = if is_custom_component {
                    &mut add_to.props
                } else {
                    &mut add_to.attrs
                };

                if let Some(list) = add_to_list.as_mut() {
                    list.push(kv);
                } else {
                    *add_to_list = Some(vec![kv])
                }
            }
            Self::Bind(key, value) => {
                let kv = (key.clone(), js::add_vm_references(p, &value.0, &value.1));

                let add_to_list = if is_custom_component {
                    &mut add_to.props
                } else {
                    &mut add_to.attrs
                };

                if let Some(list) = add_to_list.as_mut() {
                    list.push(kv);
                } else {
                    *add_to_list = Some(vec![kv])
                }
            }
            Self::On(key, value) => {
                let kv = (key.clone(), js::add_vm_references(p, &value.0, &value.1));

                if let Some(on) = add_to.on.as_mut() {
                    on.push(kv);
                } else {
                    add_to.on = Some(vec![kv]);
                }
            }
            Self::Text(_) => todo("v-text"),
            Self::Html(_) => todo("v-html"),
            Self::Show(_) => todo("v-show"),
            Self::If(_) => todo("v-if"),
            Self::Else => todo("v-else"),
            Self::ElseIf(_) => todo("v-else-if"),
            Self::For(_) => todo("v-for"),
            Self::Model(_) => todo("v-model"),
            Self::Slot(_) => todo("v-slot"),
            Self::Pre(_) => todo("v-pre"),
            Self::Cloak(_) => todo("v-cloak"),
            Self::Once => todo("v-once"),
        }
    }
    pub fn key_eq(&self, parser: &Parser, key: &str) -> bool {
        match self {
            Self::Default(key_location, _) => key_location.eq(parser, key.chars()),
            Self::Bind(key_location, _) => {
                if key.starts_with(':') {
                    key_location.eq(parser, key.chars().skip(1))
                } else if key.starts_with("v-bind:") {
                    key_location.eq(parser, key.chars().skip(7))
                } else {
                    key_location.eq(parser, key.chars())
                }
            }
            Self::On(key_location, _) => {
                if key.starts_with('@') {
                    key_location.eq(parser, key.chars().skip(1))
                } else if key.starts_with("v-on:") {
                    key_location.eq(parser, key.chars().skip(5))
                } else {
                    key_location.eq(parser, key.chars())
                }
            }
            Self::Text(_) => key == "v-text",
            Self::Html(_) => key == "v-html",
            Self::Show(_) => key == "v-show",
            Self::If(_) => key == "v-if",
            Self::Else => key == "v-else",
            Self::ElseIf(_) => key == "v-else-if",
            Self::For(_) => key == "v-for",
            Self::Model(_) => key == "v-model",
            Self::Slot(_) => key == "v-slot",
            Self::Pre(_) => key == "v-pre",
            Self::Cloak(_) => key == "v-cloak",
            Self::Once => key == "v-once",
        }
    }
    pub fn value(&self) -> SourceLocation {
        match self {
            Self::Default(_, v) => v.clone().unwrap_or(SourceLocation::empty()),
            Self::On(_, v)
            | Self::Bind(_, v)
            | Self::Text(v)
            | Self::Html(v)
            | Self::Show(v)
            | Self::If(v)
            | Self::ElseIf(v)
            | Self::For(v)
            | Self::Model(v)
            | Self::Slot(v)
            | Self::Pre(v)
            | Self::Cloak(v) => v.0.clone(),
            Self::Once => SourceLocation::empty(),
            Self::Else => SourceLocation::empty(),
        }
    }
}
