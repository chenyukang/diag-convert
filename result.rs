#[derive(Diagnostic)]
pub(crate) enum UnexpectedTokenAfterStructName {
    #[diag(label = "expected `where`, `{`, `(`, or `;` after struct name, found reserved identifier `{$token}`" )]
    ReservedIdentifier {
        #[primary_span]
        #[label(parse_unexpected_token_after_struct_name)]
        span: Span,
        token: Token,
    },
}
