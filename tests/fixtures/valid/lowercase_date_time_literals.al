// AL is case-insensitive, so the D/T literal suffixes may be lowercase.
codeunit 50100 "Lowercase Literals"
{
    procedure P()
    var
        D: Date;
        T: Time;
        DT: DateTime;
    begin
        D := 0d;
        D := 20240131d;
        T := 0t;
        T := 235959.999t;
        DT := 0dt;
        DT := 20240131120000dt;
    end;
}
