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
