// Spec header from verilog-fpga
module briv_top (input wire clk, input wire rst_n);

module simple_contract (
    input logic clk,
    input logic rst_n
);

    logic signed [31:0] counter;

    // Logic for variable: counter
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            counter <= 0;
        end else begin
            if ((counter < 10)) begin
                counter <= (counter + 1);
            end
        end
    end

endmodule

