// vendored-style foreign module referenced by uart_extern.bv (Slice A)
module uart_top(
  input        clock,
  input        reset,
  input  [63:0] rx,
  output [63:0] byte_out
);
  assign byte_out = rx;
endmodule
