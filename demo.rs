#[derive(Diagnostic)]
pub(crate) enum UnexpectedTokenAfterStructName {
    #[diag(parse_unexpected_token_after_struct_name_found_reserved_identifier)]
    ReservedIdentifier {
        #[primary_span]
        #[label(parse_unexpected_token_after_struct_name)]
        span: Span,
        token: Token,
    },
}
