// test_data/ts_example/index.ts

async function delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function mainAsyncFunction(): Promise<string> {
    console.log("Starting async function...");
    await delay(50);
    console.log("...async function finished.");
    return "Done";
}

mainAsyncFunction(); 