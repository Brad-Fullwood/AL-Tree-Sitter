codeunit 50102 Invalid_Header extends
{
    // `extends` here is nonsensical for codeunit; this should not parse as a valid header.
    // Also has an extra brace at the end to force a structural failure.
    procedure Foo();
    begin
        exit();
    end;
}}

