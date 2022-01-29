use super::utils::{is_space, write_str};
use super::{js, Parser, ParserError, SourceLocation, Tag, TagType};

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
            'a'..='z' | 'A'..='Z' | '0'..='9' => {}
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
        c = match p.try_parse_arg(c)? {
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
                        arg.insert_into_js_tag_args(&mut js_tag_args, is_custom_component);
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
                js::add_vm_references(p, resp, var, global_refs);
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
    pub class: Option<SourceLocation>,

    // Same API as `v-bind:style`, accepting either
    // a string, object, or array of objects.
    //{ color: 'red', fontSize: '14px'}
    pub style: Option<SourceLocation>,

    // Normal HTML attributes
    // { foo: 'bar' }
    pub static_attrs: Option<Vec<(SourceLocation, Option<SourceLocation>)>>,
    pub js_attrs: Option<Vec<(SourceLocation, (SourceLocation, Vec<SourceLocation>))>>,

    // Component props
    // { myProp: 'bar' }
    pub static_props: Option<Vec<(SourceLocation, Option<SourceLocation>)>>,
    pub js_props: Option<Vec<(SourceLocation, (SourceLocation, Vec<SourceLocation>))>>,

    // DOM properties
    // domProps: { innerHTML: 'baz' }
    pub dom_props: Option<Vec<(SourceLocation, SourceLocation)>>,

    // Event handlers are nested under `on`, though
    // modifiers such as in `v-on:keyup.enter` are not
    // supported. You'll have to manually check the
    // keyCode in the handler instead.
    // { click: this.clickHandler }
    pub on: Option<Vec<(SourceLocation, (SourceLocation, Vec<SourceLocation>))>>,

    // For components only. Allows you to listen to
    // native events, rather than events emitted from
    // the component using `vm.$emit`.
    // nativeOn: { click: this.nativeClickHandler }
    pub native_on: Option<Vec<(SourceLocation, SourceLocation)>>,

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
            static_attrs: None,
            js_attrs: None,
            static_props: None,
            js_props: None,
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

        if self.static_attrs.is_some() || self.js_attrs.is_some() {
            object_entries.add(dest);
            write_str("attrs:{", dest);
            let mut attrs_entries = CommaSeperatedEntries::new();

            if let Some(attrs) = self.static_attrs.as_ref() {
                for (key, value) in attrs {
                    attrs_entries.add(dest);

                    dest.push('"');
                    key.write_to_vec_escape(p, dest, '"', '\\');
                    write_str("\":", dest);

                    if let Some(value) = value {
                        dest.push('"');
                        value.write_to_vec_escape(p, dest, '"', '\\');
                        dest.push('"');
                    } else {
                        write_str("true", dest);
                    }
                }
            }

            if let Some(attrs) = self.js_attrs.as_ref() {
                for (key, value) in attrs {
                    attrs_entries.add(dest);

                    dest.push('"');
                    key.write_to_vec_escape(p, dest, '"', '\\');
                    write_str("\":", dest);

                    js::add_vm_references(p, dest, &value.0, &value.1);
                }
            }

            dest.push('}');
        }

        if self.static_props.is_some() || self.js_props.is_some() {
            object_entries.add(dest);
            write_str("props:{", dest);
            let mut props_entries = CommaSeperatedEntries::new();

            if let Some(props) = self.static_props.as_ref() {
                for (key, value) in props {
                    props_entries.add(dest);

                    dest.push('"');
                    key.write_to_vec_escape(p, dest, '"', '\\');
                    write_str("\":", dest);

                    if let Some(value) = value {
                        dest.push('"');
                        value.write_to_vec_escape(p, dest, '"', '\\');
                        dest.push('"');
                    } else {
                        write_str("true", dest);
                    }
                }
            }

            if let Some(attrs) = self.js_props.as_ref() {
                for (key, value) in attrs {
                    props_entries.add(dest);

                    dest.push('"');
                    key.write_to_vec_escape(p, dest, '"', '\\');
                    write_str("\":", dest);

                    js::add_vm_references(p, dest, &value.0, &value.1);
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

                js::add_vm_references(p, dest, &value.0, &value.1);

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
