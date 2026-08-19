// An unterminated string literal must still be reported as a parse error.
codeunit 50100 "Unterminated String"
{
    procedure Broken()
    begin
        Message('oops);
    end;
}
