CC ?= gcc
CFLAGS ?= -Wall -Wextra -Wpedantic -std=c99 -O0 -g
LDFLAGS ?=

all: test_simple test_add test_fib

test_simple: tests/test_simple.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_simple.c interp.c $(LDFLAGS)

test_add: tests/test_add.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_add.c interp.c $(LDFLAGS)

test_fib: tests/test_fib.c interp.c interp.h
	$(CC) $(CFLAGS) -o $@ tests/test_fib.c interp.c $(LDFLAGS)

check: all
	./test_simple
	./test_add
	./test_fib

clean:
	rm -f test_simple test_add test_fib

.PHONY: all check clean
