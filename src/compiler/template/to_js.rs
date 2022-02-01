use super::super::utils::write_str;
use super::super::{js, Parser};
use super::{Child, VueTagArgs};

/*
_c('div', [
    _c('h1', [
        _vm._v(\"It wurks \" + _vm._s(_vm.count) + \" !\")
    ]),
    _c('button', { on: { \"click\": $event => { _vm.count++ } } }, [_vm._v(\"+\")]),
    _c('button', { on: { \"click\": $event => { _vm.count-- } } }, [_vm._v(\"-\")]),
])
*/

const DEFAULT_CONF: &'static str = "
__vue_2_file_default_export__.render = c => {
    const _vm = this;
    const _h = _vm.$createElement;
    const _c = _vm._self._c || _h;
    return ";

pub fn template_to_js(p: &Parser, resp: &mut Vec<char>) {
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
            let child = template.content.get(0).unwrap();
            child_to_js(&child, p, resp);
        }
        content_len => {
            resp.push('[');
            for (idx, child) in template.content.iter().enumerate() {
                child_to_js(child, p, resp);
                if idx + 1 != content_len {
                    resp.push(',');
                }
            }
            resp.push(']');
        }
    }

    resp.append(&mut "\n};".chars().collect())
}

pub fn child_to_js(child: &Child, p: &Parser, resp: &mut Vec<char>) {
    // TODO support html escape
    match child {
        Child::Tag(tag, children) => {
            // Writes:
            // _c('div', [_c(..), _c(..)])
            write_str("_c('", resp);
            tag.name.write_to_vec_escape(p, resp, '\'', '\\');
            resp.push('\'');
            if tag.args.has_js_component_args {
                let is_custom_component = tag.is_custom_component(p);
                resp.push(',');
                vue_tag_args_to_js(&tag.args, resp, is_custom_component);
            }

            write_str(",[", resp);
            let children_len = children.len();
            if children_len != 0 {
                let children_max_idx = children_len - 1;
                for (idx, child) in children.iter().enumerate() {
                    child_to_js(child, p, resp);
                    if idx != children_max_idx {
                        resp.push(',');
                    }
                }
            }
            write_str("])", resp);
        }
        Child::Text(location) => {
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
        Child::Var(var, global_refs) => {
            write_str("_vm._s(", resp);
            write_str(&js::add_vm_references(p, var, global_refs), resp);
            resp.push(')');
        }
    }
}

pub fn vue_tag_args_to_js(args: &VueTagArgs, dest: &mut Vec<char>, is_custom_component: bool) {
    dest.push('{');
    let mut object_entries = CommaSeparatedEntries::new();

    if let Some(class) = args.class.as_ref() {
        object_entries.add(dest);
        write_str("class:", dest);
        write_str(&class, dest);
    }

    if let Some(style) = args.style.as_ref() {
        object_entries.add(dest);
        write_str("style:", dest);
        write_str(&style, dest);
    }

    if let Some(attrs) = args.attrs_or_props.as_ref() {
        object_entries.add(dest);
        if is_custom_component {
            write_str("props:{", dest);
        } else {
            write_str("attrs:{", dest);
        }
        let mut attrs_entries = CommaSeparatedEntries::new();

        for (key, value) in attrs {
            attrs_entries.add(dest);

            dest.push('"');
            write_str(&js::escape_quotes(key, '"'), dest);
            write_str("\":", dest);

            for c in value.chars() {
                dest.push(c);
            }
        }

        dest.push('}');
    }

    // TODO: dom_props // Option<Vec<(SourceLocation, SourceLocation)>>,

    if let Some(on) = args.on.as_ref() {
        object_entries.add(dest);
        write_str("on:{", dest);
        let mut on_entries = CommaSeparatedEntries::new();

        for (key, value) in on {
            on_entries.add(dest);

            dest.push('"');
            write_str(&js::escape_quotes(key, '"'), dest);
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

    if let Some(slot) = args.slot.as_ref() {
        object_entries.add(dest);
        write_str("slot:", dest);
        write_str(&slot, dest);
    }

    if let Some(key) = args.key.as_ref() {
        object_entries.add(dest);
        write_str("key:", dest);
        write_str(&key, dest);
    }

    if let Some(ref_) = args.ref_.as_ref() {
        object_entries.add(dest);
        write_str("ref:", dest);
        write_str(&ref_, dest);
    }

    // TODO: ref_in_for // Option<bool>,
    dest.push('}');
}

struct CommaSeparatedEntries(bool);

impl CommaSeparatedEntries {
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
