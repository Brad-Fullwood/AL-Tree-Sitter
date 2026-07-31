codeunit 50104 "Empty Control Bodies"
{
    procedure Validate()
    var
        i: Integer;
    begin
        while i < 0 do;
        for i := 1 to 0 do;
        foreach i in Numbers do;
        with Rec do;
        asserterror;
    end;
}
