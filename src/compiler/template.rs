use super::{js, utils::is_space, Parser, ParserError, SourceLocation, Tag, TagType};

pub fn compile(p: &mut Parser) -> Result<Vec<Child>, ParserError> {
    let mut compile_result = Child::compile_children(p, &mut Vec::new())?;
    loop {
        if compile_result.1.eq(p, "template".chars()) {
            return Ok(compile_result.0);
        } else {
            compile_result = Child::compile_children(p, &mut Vec::new())?;
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
    fn compile_children(
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
                    let tag = p.parse_tag()?;
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
                                Self::compile_children(p, parents_tag_names);

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
                    };
                }
                CompileAfterTextNode::Var => {
                    resp.push(Self::compile_var(p)?);
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

    fn compile_var(p: &mut Parser) -> Result<Self, ParserError> {
        let start = p.current_char;
        let global_vars = js::compile_template_var(p)?;
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
                resp.push('_');
                resp.push('c');
                resp.push('(');
                resp.push('\'');
                for c in tag.name.chars(p).iter() {
                    resp.push(*c);
                }
                resp.push('\'');
                if tag.args.len() != 0 {
                    let is_custom_component = tag.is_custom_component(p);

                    let mut js_tag_args = JsTagArgs::new();
                    for arg in tag.args.iter() {
                        arg.insert_into_js_tag_args(&mut js_tag_args, is_custom_component);
                    }
                    /* TODO convert js_tag_args into a js object */
                }

                resp.push(',');
                resp.push('[');
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
                resp.push(']');
                resp.push(')');
            }
            Self::Text(location) => {
                // Writes:
                // _vm._v("foo bar")
                resp.push('_');
                resp.push('v');
                resp.push('m');
                resp.push('.');
                resp.push('_');
                resp.push('v');
                resp.push('(');
                resp.push('"');
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
                resp.push('"');
                resp.push(')');
            }
            Self::Var(var, global_refs) => todo!("var"),
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
            resp.push('[');
            resp.push(']');
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
    pub js_attrs: Option<Vec<(SourceLocation, SourceLocation)>>,

    // Component props
    // { myProp: 'bar' }
    pub static_props: Option<Vec<(SourceLocation, Option<SourceLocation>)>>,
    pub js_props: Option<Vec<(SourceLocation, SourceLocation)>>,

    // DOM properties
    // domProps: { innerHTML: 'baz' }
    pub dom_props: Option<Vec<(SourceLocation, SourceLocation)>>,

    // Event handlers are nested under `on`, though
    // modifiers such as in `v-on:keyup.enter` are not
    // supported. You'll have to manually check the
    // keyCode in the handler instead.
    // { click: this.clickHandler }
    pub on: Option<Vec<(SourceLocation, SourceLocation)>>,

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
}

#[derive(Debug)]
pub struct JsTagArgsDirective {
    pub name: String,                   // "my-custom-directive"
    pub value: String,                  // "2"
    pub expression: String,             // "1 + 1"
    pub arg: String,                    // "foo",
    pub modifiers: Vec<(String, bool)>, // { bar: true }
}
