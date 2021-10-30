import { engine } from "./engine"
const myArr = [...Array(512 * 4).keys()].map((x, i) => {
    return {value: 0, basis: i % 16, data: [...Array(512 * 20).keys()]}
})
const opArr = [...Array(100).keys()]
let operations = 0

const recursion = (depth = 0) => {
    if (depth >= 10) return 
    recursion(depth + 1)
}

const job = (tick: number) => {
    myArr.forEach((value, index) => {
        opArr.map(x => {
            operations += 1
            recursion()
            value.value = Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick) + Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick)+ Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick) + Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick)
            value.value += Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick) / Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick) * Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick) + Math.sin(value.basis / 2 + tick) + Math.cos(value.basis / 2 + tick)
        })
    })
}

export const speedtest = () => {
    engine.registerTickAction(job)
    engine.start()
    setInterval(() => {
        console.log("REPORT", operations, engine.getCurrentFps(), engine.reportHistory())
    }, 1000)
}