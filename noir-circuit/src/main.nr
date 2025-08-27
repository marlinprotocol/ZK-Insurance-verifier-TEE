fn main(age: u32, bmi: u32, min_age: pub u32, max_age: pub u32, min_bmi: pub u32, max_bmi: pub u32) {
    // Age verification: 10 <= age <= 25
    assert(age >= min_age);
    assert(age <= max_age);
    
    // BMI verification: 18.5 <= BMI <= 24.9
    // Since Noir doesn't support floating point, we multiply by 10
    // So 18.5 becomes 185 and 24.9 becomes 249
    assert(bmi >= min_bmi);
    assert(bmi <= max_bmi);
}