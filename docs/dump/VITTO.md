U sure about wanting a unified central reseracher with general knowledge on cross-knowledge could be a good idea. at least a way to cross reference project must be there. A reseracher never reasons about project independently I feel. 

## Features I(Vittorio) find useful: TECHNICAL and INFRASTRUCTURAL, not about reserach design, mainly about experiment 
In my workflow I usally find I do three things: 
1. modify the code to run a new experiment
    * **imperfect code**: research code doesn't need to be perfect. Only when an experiment needs to be replicated needs to be improved
    * **debug runs**: when doing this often I run debug runs to check everything seems correct about result.
    * **partial results**: i always find I have to explicitely tell the model to write partial results, these are fundamental for a model to understand if an experiment and it's code are correct

    GIVEN THE ABOVE: 
    - I imagine an agent for experiment that has a goal to produce one or more metrics out of the experiment and he by himself in the background works to get those done. 
    - It's independent from the rest of the agent, I don't want to 
2. dispatch job with different hyperparameters to run the experiemtn -> OFTEN doesn't require writing code: just ablations. 
    * here very often I find myself waiting and having to prompt the model to retrieve the results once finished. If I do something else in teh meantime he forgets he run those experiemnts and it's a lot of tokens to add context in there again
3. Create tables and writing results. 
    * this simple if results are written and stored correctly

- experiment tracker: I am using an .md file with tables that track experiment ID, if run or not, and some specific on what hyperarmas. 

## Drawbacks of current systems
- I feel like if they have in context even a mention of a method you tested and it did not work, they keep proposing it. We should be focus a lot on this aspect
- They struggle a lot with linking ideas together. 

