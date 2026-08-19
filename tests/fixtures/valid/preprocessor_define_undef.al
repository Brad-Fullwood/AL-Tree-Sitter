// An in-source #define must be visible to a later #if, and #undef must undo it.
#define FEATURE_ON
#define FEATURE_OFF
#undef FEATURE_OFF
codeunit 50100 "Preprocessor Defines"
{
    procedure P()
    begin
#if FEATURE_ON
        Message('active');
#endif
#if FEATURE_OFF
        this line is not valid AL and must stay excluded
#endif
    end;
}
