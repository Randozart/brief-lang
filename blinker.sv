module blinker (
    input logic clk,
    input logic rst_n,
    output logic  led // pin: P11
);

    logic [31:0] counter;

    // Logic for variable: led
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            led <= 1'b0;
        end else begin
            if ((counter == 100000000)) begin
                led <= !led;
            end
        end
    end

    // Logic for variable: counter
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            counter <= 0;
        end else begin
            if ((counter == 100000000)) begin
                counter <= 0;
            end
            else if ((counter < 100000000)) begin
                counter <= (counter + 1);
            end
        end
    end

endmodule
