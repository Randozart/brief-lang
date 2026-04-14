module vector_test (
    input logic clk,
    input logic rst_n
);

    logic [31:0] pixels [0:1023];
    logic  enabled;

    // Logic for variable: pixels
    genvar i;
    generate
        for (i = 0; i < 1024; i = i + 1) begin : pixels_logic
            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    pixels[i] <= 0;
                end else begin
                    if (enabled) begin
                        pixels[i] <= (pixels[i] + 1);
                    end
                end
            end
        end
    endgenerate

    // Logic for variable: enabled
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            enabled <= 1'b1;
        end else begin
        end
    end

endmodule
