// linked_list_c — C reference for linked_list.bv
#include <stdlib.h>
#include <stdio.h>

typedef struct Node { long val; struct Node* next; } Node;

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    Node* head = NULL;
    long count = 0, sum = 0;
    for (; count < N; ) {
        Node* node = malloc(sizeof(Node));
        node->val = count;
        node->next = head;
        head = node;
        sum += node->val;
        count++;
        if (count % 5000000 == 0)
            fprintf(stdout, "%ld\n", sum);
    }
    while (head) { Node* t = head->next; free(head); head = t; }
    return 0;
}
