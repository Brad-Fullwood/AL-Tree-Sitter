pageextension 50102 "Movement Directives" extends "Customer Card"
{
    layout
    {
        movefirst(General; Name, Address);
        moveafter(Address; County);
        movebefore("Balance (LCY)"; "Credit Limit (LCY)");
        movelast(General; "Last Date Modified");
    }
}
