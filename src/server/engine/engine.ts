export type EngineTickAction = (tick: number, currentFPS: number, timeStep: number) => void
const ONE_SECOND = 1000
class Engine {
    private fps = 40
    private currentFPS = 40
    private currentTick = 0
    
    private timeStep = 0
    private targetTimeStep = 0
    private aggregatedTime = 0
    private reportInTicks = 0

    private loadHistory = [...Array(5).keys()].map(x => 0)
    private loadHistoryPtr = 0
    private historySeconds = 30

    private actions : EngineTickAction[] = []
    private interval: NodeJS.Timeout | null = null

    private running = false

    constructor(fps = 40, historySeconds = 300) {
        this.fps = fps
        this.currentFPS = fps
        this.timeStep = ONE_SECOND / fps
        this.targetTimeStep = ONE_SECOND / fps
        this.historySeconds = historySeconds
        this.reportInTicks = fps
    }

    public registerTickAction(action: EngineTickAction) {
        this.actions.push(action)
    }

    public tick = () => {
        if(this.running) throw new Error('tick-mutex')
        this.running = true

        // Execute Tick
        const started = new Date().getTime()
        this.currentTick += 1
        for(let i = 0; i < this.actions.length; i ++ ) {
            this.actions[i](this.currentTick, this.currentFPS, this.timeStep)
        }
        const end = new Date().getTime()

        const duration = end - started
        
        // Calculate Stats
        this.aggregatedTime += duration

        if(this.reportInTicks <= 0) {
            this.loadHistory[this.loadHistoryPtr] = this.aggregatedTime / ONE_SECOND * 100
            this.loadHistoryPtr++;
            if(this.loadHistoryPtr >= this.loadHistory.length) {
                this.loadHistoryPtr = 0
            }

            this.timeStep = this.aggregatedTime / this.fps
            this.currentFPS = ONE_SECOND / this.timeStep
            this.aggregatedTime = 0
            this.reportInTicks = this.fps
        } else {
            this.reportInTicks -= 1
        }

        this.running = false
    }

    public reportHistory() {
        return this.loadHistory
    }

    public getCurrentFps() {
        return this.currentFPS
    }

    public reset() {
        this.currentFPS = this.fps
        this.timeStep = ONE_SECOND / this.fps
        this.targetTimeStep = ONE_SECOND / this.fps
        this.historySeconds
        this.loadHistory = []
        this.aggregatedTime = 0
        this.reportInTicks = this.fps
        this.actions = []
    }

    public start() {
        this.interval = setInterval(this.tick, this.targetTimeStep / 2)
    }

    public stop() {
        if(this.interval !== null) {
            clearInterval(this.interval)
            this.interval = null
        }
    }
}

export const engine = new Engine()