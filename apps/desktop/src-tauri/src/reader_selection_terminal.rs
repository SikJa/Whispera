//! Bounded Windows Terminal fallback. This policy also runs without desktop APIs in tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure { Unavailable, Empty, Password, Invalid, Changed, Timeout, Ambiguous, TerminalEmpty }
impl Failure {
    pub fn message(self) -> String {
        match self {
            Self::Unavailable => "Seleccioná texto en una aplicación compatible y volvé a pulsar el atajo. También podés pegarlo en Lectura.",
            Self::Empty => "No hay texto seleccionado. Seleccioná un fragmento y pulsá el atajo.",
            Self::Password => "No se leen campos de contraseña.",
            Self::Invalid => "Seleccioná hasta 50.000 caracteres para leer.",
            Self::Changed => "La ventana cambió. Seleccioná el texto otra vez.",
            Self::Timeout => "La aplicación tardó demasiado en entregar la selección. Volvé a pulsar el atajo.",
            Self::Ambiguous => "Hay varias selecciones en la terminal. Dejá seleccionado texto en un solo panel y volvé a pulsar el atajo.",
            Self::TerminalEmpty => "Windows Terminal no entregó texto seleccionado. Probá seleccionar con Shift mientras arrastrás y volvé a pulsar el atajo.",
        }.into()
    }
}
pub(super) fn is_terminal_class(class: &str) -> bool { class == "CASCADIA_HOSTING_WINDOW_CLASS" }

// Preserve the existing path and all hard rejections. Only an unavailable/empty
// focused selection in Windows Terminal may trigger the additional search.
pub(super) fn resolve(
    focused: Result<String, Failure>, terminal: bool, fallback: impl FnOnce() -> Result<String, Failure>,
) -> Result<String, Failure> {
    match focused {
        Err(Failure::Unavailable | Failure::Empty) if terminal => fallback(),
        other => other,
    }
}
pub(super) enum Candidate { Container, Hidden, Terminal(Option<String>) }
pub(super) trait Tree {
    type Node;
    fn guard(&mut self) -> Result<(), Failure>;
    fn candidate(&mut self, node: &Self::Node) -> Result<Candidate, Failure>;
    fn children(&mut self, node: &Self::Node, limit: usize) -> Result<Vec<Self::Node>, Failure>;
}
const MAX_NODES: usize = 128;
const MAX_DEPTH: usize = 24;
pub(super) fn selected<T: Tree>(tree: &mut T, root: T::Node) -> Result<String, Failure> {
    let mut pending = vec![(root, 0)];
    let mut visited = 0;
    let mut selection = None;
    while let Some((node, depth)) = pending.pop() {
        tree.guard()?;
        visited += 1;
        if visited > MAX_NODES || depth > MAX_DEPTH { return Err(Failure::Unavailable); }
        match tree.candidate(&node)? {
            Candidate::Hidden => {},
            Candidate::Container => {
                let remaining = MAX_NODES - visited - pending.len();
                let children = tree.children(&node, remaining)?;
                if children.len() > remaining { return Err(Failure::Unavailable); }
                pending.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
            }
            Candidate::Terminal(Some(text)) if !text.trim().is_empty() => {
                super::validate_selection(&text).map_err(|_| Failure::Invalid)?;
                if selection.is_some() { return Err(Failure::Ambiguous); }
                selection = Some(text);
            }
            Candidate::Terminal(_) => {},
        }
    }
    // Discard a candidate if focus changed or time ran out during the final call.
    tree.guard()?;
    selection.ok_or(Failure::TerminalEmpty)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Node { children: Vec<usize>, selection: Option<String>, terminal: bool, hidden: bool, fail: Option<Failure> }
    impl Node {
        fn branch(children: &[usize]) -> Self { Self { children: children.to_vec(), selection: None, terminal: false, hidden: false, fail: None } }
        fn text(text: &str) -> Self { Self { selection: Some(text.into()), terminal: true, ..Self::branch(&[]) } }
    }
    struct Fixture { nodes: Vec<Node>, observed: Vec<usize>, remaining_checks: Option<usize>, expired: Failure }
    impl Fixture {
        fn new(nodes: Vec<Node>) -> Self { Self { nodes, observed: vec![], remaining_checks: None, expired: Failure::Changed } }
    }
    impl Tree for Fixture {
        type Node = usize;
        fn guard(&mut self) -> Result<(), Failure> {
            if let Some(remaining) = self.remaining_checks.as_mut() {
                if *remaining == 0 { return Err(self.expired); }
                *remaining -= 1;
            }
            Ok(())
        }
        fn candidate(&mut self, node: &usize) -> Result<Candidate, Failure> {
            self.observed.push(*node);
            let item = &self.nodes[*node];
            if let Some(error) = item.fail { return Err(error); }
            Ok(if item.hidden { Candidate::Hidden } else if item.terminal { Candidate::Terminal(item.selection.clone()) } else { Candidate::Container })
        }
        fn children(&mut self, node: &usize, _: usize) -> Result<Vec<usize>, Failure> { Ok(self.nodes[*node].children.clone()) }
    }
    #[test]
    fn terminal_fallback_keeps_successful_and_nonterminal_reads_unchanged() {
        assert!(is_terminal_class("CASCADIA_HOSTING_WINDOW_CLASS"));
        for class in ["ConsoleWindowClass", "Chrome_WidgetWin_1", "Notepad", ""] { assert!(!is_terminal_class(class)); }
        assert_eq!(resolve(Ok("Focused selection".into()), true, || panic!("successful selection must not search other panes")), Ok("Focused selection".into()));
        for error in [Failure::Unavailable, Failure::Empty, Failure::Password, Failure::Invalid, Failure::Changed, Failure::Timeout] {
            assert_eq!(resolve(Err(error), false, || panic!("ordinary applications must keep their original result")), Err(error));
        }
        for error in [Failure::Password, Failure::Invalid, Failure::Changed, Failure::Timeout, Failure::TerminalEmpty, Failure::Ambiguous] {
            assert_eq!(resolve(Err(error), true, || panic!("hard rejection must not use another selection")), Err(error));
        }
        for error in [Failure::Unavailable, Failure::Empty] {
            assert_eq!(resolve(Err(error), true, || Ok("Terminal selection".into())), Ok("Terminal selection".into()));
        }
    }
    #[test]
    fn terminal_fallback_finds_nested_selection_after_empty_focused_control() {
        let mut fixture = Fixture::new(vec![Node::branch(&[1,2]), Node::text(""), Node::branch(&[3]), Node::text("Texto seleccionado en WSL")]);
        assert_eq!(selected(&mut fixture, 0), Ok("Texto seleccionado en WSL".into()));
    }
    #[test]
    fn terminal_fallback_ignores_hidden_tabs_and_rejects_ambiguous_visible_panes() {
        let mut hidden = Node::branch(&[3]); hidden.hidden = true;
        let mut fixture = Fixture::new(vec![Node::branch(&[1,2]), hidden, Node::text("Visible"), Node::text("Private inactive tab")]);
        assert_eq!(selected(&mut fixture, 0), Ok("Visible".into()));
        assert!(!fixture.observed.contains(&3), "hidden tab text must never be inspected");
        for second in ["Different selection", "Visible"] {
            fixture.nodes[1] = Node::text(second);
            assert_eq!(selected(&mut fixture, 0), Err(Failure::Ambiguous));
        }
    }
    #[test]
    fn terminal_fallback_rejects_empty_invalid_password_and_failed_providers() {
        for text in ["", " \r\n"] {
            let mut fixture = Fixture::new(vec![Node::text(text)]);
            assert_eq!(selected(&mut fixture, 0), Err(Failure::TerminalEmpty));
        }
        for text in ["bad\0text".to_string(), "😀".repeat(25001)] {
            let mut fixture = Fixture::new(vec![Node::text(&text)]);
            assert_eq!(selected(&mut fixture, 0), Err(Failure::Invalid));
        }
        for error in [Failure::Password, Failure::Unavailable, Failure::Ambiguous] {
            let mut broken = Node::text("Never accepted"); broken.fail = Some(error);
            let mut fixture = Fixture::new(vec![Node::branch(&[1,2]), Node::text("Valid earlier candidate"), broken]);
            assert_eq!(selected(&mut fixture, 0), Err(error), "must discard a partial result when another provider fails");
        }
    }
    #[test]
    fn terminal_fallback_discards_selection_after_focus_change_or_timeout() {
        for expired in [Failure::Changed, Failure::Timeout] {
            let mut fixture = Fixture::new(vec![Node::text("Selected before focus changed")]);
            fixture.remaining_checks = Some(1); fixture.expired = expired;
            assert_eq!(selected(&mut fixture, 0), Err(expired));
        }
    }
    #[test]
    fn terminal_fallback_does_not_accept_partial_selection_beyond_search_limits() {
        let mut fixture = Fixture::new(vec![Node::branch(&[1,2]), Node::text("Earlier candidate"), Node::branch(&(3..MAX_NODES+2).collect::<Vec<_>>())]);
        fixture.nodes.extend((3..MAX_NODES+2).map(|_| Node::branch(&[])));
        assert_eq!(selected(&mut fixture, 0), Err(Failure::Unavailable));
        let mut nodes: Vec<_> = (0..MAX_DEPTH+2).map(|i| Node::branch(&[i+1])).collect();
        nodes.push(Node::text("Too deep"));
        let mut fixture = Fixture::new(nodes);
        assert_eq!(selected(&mut fixture, 0), Err(Failure::Unavailable));
    }
}
