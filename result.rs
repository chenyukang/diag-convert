#[derive(Diagnostic)]
#[diag(text = "unexpected `=` after inclusive range")]
#[note("inclusive ranges end with a single equals sign (`..=`)")]
pub(crate) struct InclusiveRangeExtraEquals {
    #[primary_span]
    #[suggestion(
        label = "use `..=` instead" ,
        style = "short",
        code = "..=",
        applicability = "maybe-incorrect"
    )]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag(text = "unexpected `>` after inclusive range")]
pub(crate) struct InclusiveRangeMatchArrow {
    #[primary_span]
    pub arrow: Span,
    #[label("this is parsed as an inclusive range `..=`")]
    pub span: Span,
    #[suggestion(label = "add a space between the pattern and `=>`", style = "verbose", code = " ", applicability = "machine-applicable")]
    pub after_pat: Span,
}

#[derive(Diagnostic)]
#[diag(label = "inclusive range with no end" , code = "E0586")]
#[note("inclusive ranges must be bounded at the end (`..=b` or `a..=b`)")]
pub(crate) struct InclusiveRangeNoEnd {
    #[primary_span]
    #[suggestion(
        label = "use `..` instead" ,
        code = "..",
        applicability = "machine-applicable",
        style = "short"
    )]
    pub span: Span,
}
