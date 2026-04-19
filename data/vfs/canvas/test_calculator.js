const inputs = document.querySelectorAll('input');
const calculateBtn = document.getElementById('calculate');
const result = document.getElementById('result');

calculateBtn.addEventListener('click', () => {
    const num1 = parseFloat(inputs[0].value);
    const operator = document.getElementById('operator').value;
    const num2 = parseFloat(inputs[1].value);
    let calcResult;

    if (operator === '+') {
        calcResult = num1 + num2;
    } else if (operator === '-') {
        calcResult = num1 - num2;
    } else if (operator === '*') {
        calcResult = num1 * num2;
    } else if (operator === '/') {
        calcResult = num1 / num2;
    }

    result.textContent = 'Result: ' + calcResult;
});