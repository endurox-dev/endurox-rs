#
# View definitions for the unit tests.
#
# Compiled by the test harness with `viewc -n` (no C header emitted, the Rust
# side reaches the fields through Bvget/Bvset by name). Kept small on purpose:
# the tests only need a view whose layout size is well above the one-byte
# allocation a shrinking `tprealloc` would leave behind.
#
#type	cname		fbname	count	flag	size	null
VIEW TESTVIEW1
	long	tlong		-	1	-	-	-
	short	tshort		-	1	-	-	-
	double	tdouble		-	1	-	-	-
	char	tchar		-	1	-	-	-
	string	tstring		-	1	-	64	-
	carray	tcarray		-	1	-	32	-
END

# Layout exercised by the Rust VIEW derives, including native C/L indicators.
VIEW SERDE_VIEW
    long    id      -   1   -   -   -
    string  name    -   1   -   64  -
    long    values  -   5   C   -   -
    string  note    -   1   -   32  -
    carray  blob    -   1   L   64  -
    short   fixed   -   3   -   -   -
END

# Larger-than-64-KiB field conversions and JSON expansion regressions.
VIEW LARGE_VIEW
    string text - 4 - 32001 -
    carray blob - 4 - 32000 -
END

VIEW JSON_EXPANSION
    char long_member_name_for_json_000 - 1 - - -
    char long_member_name_for_json_001 - 1 - - -
    char long_member_name_for_json_002 - 1 - - -
    char long_member_name_for_json_003 - 1 - - -
    char long_member_name_for_json_004 - 1 - - -
    char long_member_name_for_json_005 - 1 - - -
    char long_member_name_for_json_006 - 1 - - -
    char long_member_name_for_json_007 - 1 - - -
    char long_member_name_for_json_008 - 1 - - -
    char long_member_name_for_json_009 - 1 - - -
    char long_member_name_for_json_010 - 1 - - -
    char long_member_name_for_json_011 - 1 - - -
    char long_member_name_for_json_012 - 1 - - -
    char long_member_name_for_json_013 - 1 - - -
    char long_member_name_for_json_014 - 1 - - -
    char long_member_name_for_json_015 - 1 - - -
    char long_member_name_for_json_016 - 1 - - -
    char long_member_name_for_json_017 - 1 - - -
    char long_member_name_for_json_018 - 1 - - -
    char long_member_name_for_json_019 - 1 - - -
    char long_member_name_for_json_020 - 1 - - -
    char long_member_name_for_json_021 - 1 - - -
    char long_member_name_for_json_022 - 1 - - -
    char long_member_name_for_json_023 - 1 - - -
    char long_member_name_for_json_024 - 1 - - -
    char long_member_name_for_json_025 - 1 - - -
    char long_member_name_for_json_026 - 1 - - -
    char long_member_name_for_json_027 - 1 - - -
    char long_member_name_for_json_028 - 1 - - -
    char long_member_name_for_json_029 - 1 - - -
    char long_member_name_for_json_030 - 1 - - -
    char long_member_name_for_json_031 - 1 - - -
    char long_member_name_for_json_032 - 1 - - -
    char long_member_name_for_json_033 - 1 - - -
    char long_member_name_for_json_034 - 1 - - -
    char long_member_name_for_json_035 - 1 - - -
    char long_member_name_for_json_036 - 1 - - -
    char long_member_name_for_json_037 - 1 - - -
    char long_member_name_for_json_038 - 1 - - -
    char long_member_name_for_json_039 - 1 - - -
    char long_member_name_for_json_040 - 1 - - -
    char long_member_name_for_json_041 - 1 - - -
    char long_member_name_for_json_042 - 1 - - -
    char long_member_name_for_json_043 - 1 - - -
    char long_member_name_for_json_044 - 1 - - -
    char long_member_name_for_json_045 - 1 - - -
    char long_member_name_for_json_046 - 1 - - -
    char long_member_name_for_json_047 - 1 - - -
END
