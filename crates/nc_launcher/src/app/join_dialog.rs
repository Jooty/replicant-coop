/// The small JOIN dialog over home: the refusal of the last ticket tried, and whether the field has been
/// given the focus yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JoinDialog {
    pub refusal: Option<&'static str>,
    pub focused_once: bool,
}
