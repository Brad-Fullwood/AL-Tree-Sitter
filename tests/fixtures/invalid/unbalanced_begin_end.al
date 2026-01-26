codeunit 50100 Invalid_Unbalanced
{
    procedure Foo();
    begin
        if true then
            exit();
    // Missing: end;
}

