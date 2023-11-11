#[derive(Diagnostic)]
#[diag(parse_inclusive_range_extra_equals)]
#[note]
pub(crate) struct InclusiveRangeExtraEquals {
    #[primary_span]
    #[suggestion(
        parse_suggestion_remove_eq,
        style = "short",
        code = "..=",
        applicability = "maybe-incorrect"
    )]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag(parse_inclusive_range_match_arrow)]
pub(crate) struct InclusiveRangeMatchArrow {
    #[primary_span]
    pub arrow: Span,
    #[label]
    pub span: Span,
    #[suggestion(style = "verbose", code = " ", applicability = "machine-applicable")]
    pub after_pat: Span,
}

#[derive(Diagnostic)]
#[diag(parse_inclusive_range_no_end, code = "E0586")]
#[note]
pub(crate) struct InclusiveRangeNoEnd {
    #[primary_span]
    #[suggestion(
        parse_suggestion_open_range,
        code = "..",
        applicability = "machine-applicable",
        style = "short"
    )]
    pub span: Span,
}
