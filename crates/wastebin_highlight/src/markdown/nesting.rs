//! How deep the tree is that the sanitizer builds from a rendered document.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::LazyLock;

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, ExpandedName, ParseOpts, QualName, local_name, ns};

use super::MAX_NESTING_DEPTH;

/// Input handed to the parser between two looks at the depth.
///
/// The parse can only be abandoned between chunks, so a chunk is how much a document past the
/// limit still gets to build: a kilobyte of `<div>` is two hundred levels more.
const CHUNK_BYTES: usize = 1024;

/// Return how deep the sanitizer's parser nests the elements of `html`, counting no further than
/// one past [`MAX_NESTING_DEPTH`].
///
/// This is html5ever's own tree builder, set up the way ammonia's `Builder::clean` sets it up — a
/// fragment in the context of a `div` — so the depth is that of the very tree the sanitizer
/// builds. Estimating it from the tags kept falling short of what the parser does with them: a
/// close tag in a comment, an attribute value or a `<textarea>` closes nothing, a misnested `<b>`
/// is reopened inside every element that follows it, and inside `<svg>` the rules change again.
/// Each shortfall handed the sanitizer the tree the limit exists to refuse.
///
/// The input goes in a chunk at a time and the parse is abandoned once it is too deep, so the
/// check does not build that tree either.
pub(super) fn nesting_depth(html: &str) -> usize {
    let mut parser = html5ever::parse_fragment(
        Tree::default(),
        ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Vec::new(),
        false,
    );

    let mut rest = html;
    while !rest.is_empty() {
        let (chunk, tail) = rest.split_at(rest.floor_char_boundary(CHUNK_BYTES));
        parser.process(StrTendril::from_slice(chunk));

        let depth = parser.tokenizer.sink.sink.depth.get();
        if depth > MAX_NESTING_DEPTH {
            return depth;
        }

        rest = tail;
    }

    parser.finish()
}

/// Answer for the name of anything that is not an element, which the tree builder never asks.
static NO_NAME: LazyLock<QualName> = LazyLock::new(|| QualName::new(None, ns!(), local_name!("")));

type Handle = Rc<Node>;

/// A node of the tree [`nesting_depth`] builds: where it hangs, and none of what it holds.
#[derive(Default)]
struct Node {
    /// `None` for what is not an element: the document, a template's contents, a comment.
    name: Option<QualName>,
    /// Asked by the tree builder, which parses the contents of one differently.
    annotation_xml_integration_point: bool,
    /// Hung beneath the template: they are a fragment of their own, but parsed inside it.
    template_contents: Option<Handle>,
    parent: RefCell<Weak<Node>>,
    /// Kept for moving them elsewhere; their order does not matter here.
    children: RefCell<Vec<Weak<Node>>>,
}

impl Node {
    /// How many elements `self` is, or is inside of, counting no further than one past the limit.
    fn level(self: &Rc<Self>) -> usize {
        let mut level = 0;
        let mut node = Some(Rc::clone(self));

        while let Some(current) = node {
            if current.name.is_some() {
                level += 1;
                if level > MAX_NESTING_DEPTH {
                    break;
                }
            }
            node = current.parent.borrow().upgrade();
        }

        level
    }
}

/// Just enough of a DOM to tell how deep it is.
#[derive(Default)]
struct Tree {
    document: Handle,
    /// Owns every node, so that the links between them can all be weak: dropping a deep tree
    /// must not recurse once per level.
    nodes: RefCell<Vec<Handle>>,
    /// Deepest an element was inserted so far.
    depth: Cell<usize>,
}

impl Tree {
    fn add(&self, node: Node) -> Handle {
        let node = Rc::new(node);
        self.nodes.borrow_mut().push(Rc::clone(&node));
        node
    }

    fn insert(&self, parent: &Handle, child: NodeOrText<Handle>) {
        // Text opens no level.
        let NodeOrText::AppendNode(child) = child else {
            return;
        };

        // The fragment parser puts everything inside an `html` element of its own, which counts
        // in place of the child itself: a `<p>` at the top of a paste is one level deep.
        if child.name.is_some() {
            self.depth.set(self.depth.get().max(parent.level()));
        }

        *child.parent.borrow_mut() = Rc::downgrade(parent);
        parent.children.borrow_mut().push(Rc::downgrade(&child));
    }
}

/// Mirrors ammonia's `RcDom` wherever the answer steers the tree builder, so both parse alike.
impl TreeSink for Tree {
    type Handle = Handle;
    type Output = usize;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> usize {
        self.depth.get()
    }

    fn parse_error(&self, _: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        Rc::clone(&self.document)
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> ExpandedName<'a> {
        target.name.as_ref().unwrap_or(&NO_NAME).expanded()
    }

    fn create_element(&self, name: QualName, _: Vec<Attribute>, flags: ElementFlags) -> Handle {
        let element = Rc::new_cyclic(|this| Node {
            name: Some(name),
            annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
            template_contents: flags.template.then(|| {
                Rc::new(Node {
                    parent: RefCell::new(this.clone()),
                    ..Node::default()
                })
            }),
            ..Node::default()
        });

        self.nodes.borrow_mut().push(Rc::clone(&element));
        element
    }

    fn create_comment(&self, _: StrTendril) -> Handle {
        self.add(Node::default())
    }

    fn create_pi(&self, _: StrTendril, _: StrTendril) -> Handle {
        self.add(Node::default())
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        self.insert(parent, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        let parent = element.parent.borrow().upgrade();

        match parent {
            Some(parent) => self.insert(&parent, child),
            None => self.insert(prev_element, child),
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}

    fn get_template_contents(&self, target: &Handle) -> Handle {
        // Only ever asked of a template.
        target
            .template_contents
            .as_ref()
            .map_or_else(|| Rc::clone(target), Rc::clone)
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        let parent = sibling.parent.borrow().upgrade();

        if let Some(parent) = parent {
            self.insert(&parent, child);
        }
    }

    fn add_attrs_if_missing(&self, _: &Handle, _: Vec<Attribute>) {}

    fn remove_from_parent(&self, target: &Handle) {
        let parent = target.parent.replace(Weak::new()).upgrade();

        if let Some(parent) = parent {
            parent
                .children
                .borrow_mut()
                .retain(|child| child.as_ptr() != Rc::as_ptr(target));
        }
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let children = std::mem::take(&mut *node.children.borrow_mut());

        for child in children.iter().filter_map(Weak::upgrade) {
            *child.parent.borrow_mut() = Rc::downgrade(new_parent);
        }

        new_parent.children.borrow_mut().extend(children);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &Handle) -> bool {
        handle.annotation_xml_integration_point
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_counted_rather_than_tags() {
        assert_eq!(nesting_depth(""), 0);
        assert_eq!(nesting_depth("text"), 0);
        assert_eq!(nesting_depth("<p>hi</p>"), 1);
        assert_eq!(nesting_depth("<div><p>hi</p></div>"), 2);
        assert_eq!(nesting_depth("<p>a</p><p>b</p>"), 1);
        assert_eq!(nesting_depth("<br><br><br>"), 1);
        assert_eq!(nesting_depth("<img src=\"x\"/>"), 1);
        // A slash cannot close a `div`, so these nest rather than sit side by side.
        assert_eq!(nesting_depth("<div/><div/>"), 2);
        // Text containing a bare `<` is not markup.
        assert_eq!(nesting_depth("1 < 2"), 0);
    }

    /// Every one of these nests a `div` per repetition, behind something that read as closing it
    /// to a scan of the tags but not to the parser.
    #[test]
    fn nesting_is_measured_however_the_markup_hides_it() {
        for unit in [
            // A close tag naming nothing that is open is dropped.
            "<div></span>",
            // A slash closes only a void element.
            "<div/>",
            "<div><!-- </div> -->",
            "<div><!x </div>>",
            "<div><? </div>>",
            r#"<div title="</div>">"#,
            "<div title='</div>'>",
            "<div title=</div>>",
            "<div </div>>",
            "<div><textarea></div></textarea>",
            "<div><title></div></title>",
            "<div><style></div></style>",
            // A `<script>` opened inside an escaped script does not end at the first close tag.
            "<div><script><!--<script></script></div></script>",
            "<div><template>",
            // A close tag with a scope boundary between it and the element it names is dropped,
            // and the sanitizer then strips the boundary and keeps what it held.
            "<div><object></div>",
            "<div><applet></div>",
            "<div><marquee></div>",
        ] {
            let html = unit.repeat(MAX_NESTING_DEPTH + 10);
            let depth = nesting_depth(&html);
            assert!(depth > MAX_NESTING_DEPTH, "{unit:?} measured {depth} deep");
        }
    }

    /// An unclosed formatting element is reopened inside every element that follows it, so this
    /// nests one level deeper each time though no tag is left open. Only three alike are kept to
    /// reopen, hence the differing attribute.
    #[test]
    fn reopened_formatting_elements_are_measured() {
        let html: String = (0..MAX_NESTING_DEPTH + 10)
            .map(|i| format!("<div><b class={i}></div>"))
            .collect();

        assert!(nesting_depth(&html) > MAX_NESTING_DEPTH);
    }

    /// Past the limit the parse stops, rather than going on to build all of a tree it refuses.
    #[test]
    fn the_count_stops_one_past_the_limit() {
        assert_eq!(
            nesting_depth(&"<div>".repeat(100_000)),
            MAX_NESTING_DEPTH + 1
        );
    }

    /// The depth is only that of the sanitizer's tree while both come from the same html5ever,
    /// and nothing else would notice ammonia moving to a different one.
    #[test]
    fn the_sanitizer_parses_with_the_same_html5ever() {
        let lock = include_str!("../../../../Cargo.lock");
        assert_eq!(lock.matches("name = \"html5ever\"\n").count(), 1);
    }
}
