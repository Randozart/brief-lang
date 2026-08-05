CC ?= gcc
CFLAGS ?= -Wall -Wextra -Wpedantic -std=c99 -O0 -g
LDFLAGS ?=

all: tamer test_simple test_add test_from_brivc test_briv_tamer

test_simple: tests/test_simple.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_simple.c interp.c $(LDFLAGS)

test_add: tests/test_add.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_add.c interp.c $(LDFLAGS)

test_from_brivc: tests/test_from_brivc.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_from_brivc.c interp.c $(LDFLAGS)

test_briv_tamer: tests/test_briv_tamer.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_briv_tamer.c interp.c $(LDFLAGS)

tamer: main.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ main.c interp.c $(LDFLAGS)

check: all
	./test_simple
	./test_add
	./test_from_brivc /tmp/user_add.lair
	./test_briv_tamer

clean:
	rm -f test_simple test_add test_from_brivc test_briv_tamer tamer

.PHONY: all check clean
