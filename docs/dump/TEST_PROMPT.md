# Test Prompts

## Workspace Init

Run the following to see CC populate the workspace for your research needs:
```
Hello, let's get everything set up in the IRE workspace. I am working on diffusion language models, and I want to understand whether they natively encode output length information in their latent space without explicit training. I am trying to set up LLADA 8B to run locally now.
```
Expected output: CC should populate NOTES.md, IDEAS.md, and PULSE.md with relevant material.

## Resource Ingestion

Run the following to test resource ingestion. If the workspace is initialized as above, even better.
```
# paste inside the ingestion field and click submit
https://arxiv.org/pdf/2603.06123
```
CC should read the PDF and provide a summary of the relevant information in a new chat tab. You can say confirm to index it.

## Experiment Dispatch

Run the following prompt:
```
Let's test the experiment workflow. Write a minimal Python script that, for each i in range(10), sleeps 3 seconds and prints (iteration nr. i), then finishes. To make the test more useful, dispatch two experiments. One should work and finish without error, and the other should raise an exception and fail. That way, I can inspect both behaviors.
```

## Frontend Chat Rendering

Run the following to generate multiple tools, thoughts, and text.
```
I am testing IRE's frontend rendering, which wraps your answers with a custom UI. To help, please respond with some reasoning, text, and multiple tool uses. Keep the response brief to avoid overspending tokens, but ensure variety in the content (that is, use thinking, tools, and plain text).
```