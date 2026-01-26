codeunit 50101 Invalid_BadIf
{
    procedure Foo();
    begin
        if then begin
            exit();
        end;
    end;
}

