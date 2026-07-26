codeunit 50103 "Signed Case Labels"
{
    procedure Verify(Value: Integer; Entry: Record "G/L Entry")
    begin
        case Value of
            -1:// adjacent comment boundary
                ;
            -Entry."Remaining Amt. (LCY)" > 0:
                ;
        end;
    end;
}
