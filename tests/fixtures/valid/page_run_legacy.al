codeunit 50109 "Legacy Page Run"
{
    procedure OpenPage()
    var
        Customer: Record Customer;
    begin
        PAGE.Run(0, Customer);
    end;
}
