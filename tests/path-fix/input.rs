impl<'a> Parser<'a> {
    /// Parses attributes that appear before an item.
    pub(super) fn parse_outer_attributes(&mut self) -> PResult<'a, AttrWrapper> {
        let mut outer_attrs = ast::AttrVec::new();
        let mut just_parsed_doc_comment = false;
        let start_pos = self.num_bump_calls;
        loop {
            let attr = if self.check(&token::Pound) {
                let prev_outer_attr_sp = outer_attrs.last().map(|attr| attr.span);

                let inner_error_reason = if just_parsed_doc_comment {
                    Some(InnerAttrForbiddenReason::AfterOuterDocComment {
                        prev_doc_comment_span: prev_outer_attr_sp.unwrap(),
                    })
                } else {
                    prev_outer_attr_sp.map(|prev_outer_attr_sp| {
                        InnerAttrForbiddenReason::AfterOuterAttribute { prev_outer_attr_sp }
                    })
                };
                let inner_parse_policy = InnerAttrPolicy::Forbidden(inner_error_reason);
                just_parsed_doc_comment = false;
                Some(self.parse_attribute(inner_parse_policy)?)
            } else if let token::DocComment(comment_kind, attr_style, data) = self.token.kind {
                if attr_style != ast::AttrStyle::Outer {
                    let span = self.token.span;
                    let mut err = self.sess.span_diagnostic.struct_span_err_with_code(
                        span,
                        fluent::parse_inner_doc_comment_not_permitted,
                        error_code!(E0753),
                    );
                    if let Some(replacement_span) = self.annotate_following_item_if_applicable(
                        &mut err,
                        span,
                        match comment_kind {
                            token::CommentKind::Line => OuterAttributeType::DocComment,
                            token::CommentKind::Block => OuterAttributeType::DocBlockComment,
                        },
                    ) {
                        err.note(fluent::parse_note);
                        err.span_suggestion_verbose(
                            replacement_span,
                            fluent::parse_suggestion,
                            "",
                            rustc_errors::Applicability::MachineApplicable,
                        );
                    }
                    err.emit();
                }
                self.bump();
                just_parsed_doc_comment = true;
                // Always make an outer attribute - this allows us to recover from a misplaced
                // inner attribute.
                Some(attr::mk_doc_comment(
                    &self.sess.attr_id_generator,
                    comment_kind,
                    ast::AttrStyle::Outer,
                    data,
                    self.prev_token.span,
                ))
            } else {
                None
            };

            if let Some(attr) = attr {
                if attr.style == ast::AttrStyle::Outer {
                    outer_attrs.push(attr);
                }
            } else {
                break;
            }
        }
        Ok(AttrWrapper::new(outer_attrs, start_pos))
    }
}
