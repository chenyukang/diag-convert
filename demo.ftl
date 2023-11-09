parse_invalid_char_in_escape = {parse_invalid_char_in_escape_msg}: `{$ch}`
    .label = {parse_invalid_char_in_escape_msg}

parse_invalid_char_in_escape_msg = invalid character in {$is_hex ->
    [true] numeric character
    *[false] unicode
    } escape
