/*
***********************************************************************
  CODEGEN.C : IMPLEMENT CODE GENERATION HERE
************************************************************************
*/
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
extern crate libc;
use crate::expression::*;
use std::fs::File;
use std::io::prelude::*;

pub const INVAL: i64 = -999;

/*
*************************************************************************************
 USE A STRUCTURE TO STORE GLOBAL VARIABLES
*************************************************************************************
*/
struct globals {
    // The last used offset will help determine the last used stack location
    pub last_used_offset: i64,
    pub last_offset_used: String,
    // The arg counter is used to iterate through the arg list.
    pub arg_counter: i64,
}

impl globals {
    fn new() -> Self {
        globals {
            last_used_offset: 0,
            last_offset_used: "".to_string(),
            arg_counter: 0,
        }
    }
}
/*
*************************************************************************************
     THE REGINFO LIST TRACKS IF REGISTERS ARE AVAILABLE FOR USE
**************************************************************************************
*/

#[derive(Clone)]
struct regInfo {
    name: String,
    avail: i8,
    next: LinkInfo,
}

type LinkInfo = Option<Box<regInfo>>;

impl Default for regInfo {
    fn default() -> regInfo {
        regInfo {
            name: "".to_string(),
            avail: 0,
            next: None,
        }
    }
}

struct regList {
    pub head: LinkInfo,
}

impl regList {
    fn new() -> Self {
        regList { head: None }
    }

    /*
    ***********************************************************************
      FUNCTION TO ADD NEW REGISTER INFORMATION TO THE REGISTER INFO LIST
    ************************************************************************
    */
    fn add_reg(&mut self, name: &str, avail: i8) {
        let new_node = Box::new(regInfo {
            name: name.to_string(),
            avail: avail,
            next: None,
        });

        if !self.head.is_none() {
            let mut current = &mut self.head;
            loop {
                if let Some(node) = current.as_mut() {
                    if node.next.is_none() {
                        node.next = Some(new_node);
                        break;
                    }
                    current = &mut node.next;
                } else {
                    break;
                }
            }
        } else {
            self.head = Some(new_node);
        }
    }

    /*
    ***********************************************************************
      FUNCTION TO UPDATE THE AVAILIBILITY OF REGISTERS IN THE REG INFO LIST
    ************************************************************************
    */
    fn update_reg_info(&mut self, name: String, avail: i8) {
        let mut current = &mut self.head;
        loop {
            if let Some(node) = current.as_mut() {
                if node.name == name {
                    node.avail = avail;
                }
                current = &mut node.next;
            } else {
                break;
            }
        }
    }

    /*
    ***********************************************************************
      FUNCTION TO RETURN THE NEXT AVAILABLE REGISTER
    ************************************************************************
    */
    fn get_next_avail_reg(&self, noAcc: bool) -> String {
        let mut current = &self.head;

        if current.is_none() {
            println!("List is empty");
        }

        loop {
            if let Some(node) = current.as_ref() {
                if node.avail == 1 {
                    if !noAcc {
                        return node.name.clone();
                    }
                    // if not rax and dont return accumulator set to true, return the other reg
                    // if rax and noAcc == true, skip to next avail
                    if noAcc && !(node.name == "%rax") {
                        return node.name.clone();
                    }
                }
                current = &node.next;
            } else {
                break;
            }
        }
        return "NoReg".to_string();
    }

    /*
    ***********************************************************************
      FUNCTION TO DETERMINE IF ANY REGISTER APART FROM OR INCLUDING
      THE ACCUMULATOR(RAX) IS AVAILABLE
    ************************************************************************
    */
    fn if_avail_reg(&self, noAcc: bool) -> usize {
        let mut current = &self.head;

        if current.is_none() {
            println!("Empty reglist");
        }

        loop {
            if let Some(node) = current.as_ref() {
                if node.avail == 1 {
                    // registers available
                    if !noAcc {
                        return 1;
                    } else if noAcc && !(node.name == "%rax") {
                        return 1;
                    }
                }
                current = &node.next;
            } else {
                break;
            }
        }
        return 0;
    }

    /*
    ***********************************************************************
      FUNCTION TO DETERMINE IF A SPECIFIC REGISTER IS AVAILABLE
    ************************************************************************
    */
    fn is_avail_reg(&self, name: String) -> bool {
        let mut current = &self.head;

        if current.is_none() {
            println!("Empty reglist");
        }

        loop {
            if let Some(node) = current.as_ref() {
                if node.name == name {
                    if node.avail == 1 {
                        return true;
                    }
                }
                current = &node.next;
            } else {
                break;
            }
        }
        return false;
    }

    /*
    ***********************************************************************
      FUNCTION TO FREE REGISTER INFORMATION LIST
    ************************************************************************
    */
    fn free_list(&mut self) {
        while !self.head.is_none() {
            self.head.take().map(|node| {
                self.head = node.next;
            });
        }
    }

    fn print_list(&self) {
        let mut current = &self.head;

        if current.is_none() {
            println!("Empty reglist");
        }

        loop {
            if let Some(node) = current.as_ref() {
                print!("\t {} : {} -> ", node.name, node.avail);

                current = &node.next;
            } else {
                break;
            }
        }
        println!();
    }
}

/*
*************************************************************************************
     THE VARSTOREINFO LIST TRACKS A VARIABLE NAME, VALUE AND WHERE IT IS STORED
**************************************************************************************
*/

#[derive(Clone)]
struct varStoreInfo {
    name: String,
    // FLAG TO IDENTIFY IF A VARIABLE IS A CONSTANT OR NOT.
    is_const: bool,
    value: i64,
    // LOCATION COULD BE A STACK LOCATION OR A REGISTER
    // eg: -8(%rbp) or %rcx
    location: String,
    next: LinkVar,
}

type LinkVar = Option<Box<varStoreInfo>>;

impl Default for varStoreInfo {
    fn default() -> varStoreInfo {
        varStoreInfo {
            name: "".to_string(),
            is_const: false,
            value: 0,
            location: "".to_string(),
            next: None,
        }
    }
}

struct varStList {
    head: LinkVar,
}

impl varStList {
    fn new() -> Self {
        varStList { head: None }
    }

    /*
    ***********************************************************************
      FUNCTION TO ADD VARIABLE INFORMATION TO THE VARIABLE INFO LIST
    ************************************************************************
    */
    fn add_var_info(&mut self, varname: String, location: String, val: i64, is_const: bool) {
        // push front like a stack
        let new_node = Box::new(varStoreInfo {
            name: varname,
            is_const: is_const,
            value: val,
            location: location.clone(),
            next: self.head.take(),
        });

        self.head = Some(new_node);
    }

    /*
    ***********************************************************************
      FUNCTION TO LOOKUP VARIABLE INFORMATION FROM THE VARINFO LIST
    ************************************************************************
    */
    fn lookup_var_info(&self, name: String, val: i64) -> String {
        let mut current = &self.head;

        if current.is_none() {
            println!("Empty varlist");
        }

        loop {
            if let Some(node) = current.as_ref() {
                if node.is_const == true {
                    if node.value == val {
                        return node.location.clone();
                    }
                } else {
                    if node.name == name {
                        return node.location.clone();
                    }
                }
                current = &node.next;
            } else {
                break;
            }
        }
        return "".to_string();
    }

    /*
    ***********************************************************************
      FUNCTION TO UPDATE VARIABLE INFORMATION
    ************************************************************************
    */
    fn update_var_info(&mut self, varName: String, location: String, val: i64, is_const: bool) {
        if self.lookup_var_info(varName.clone(), val) == "".to_string() {
            self.add_var_info(varName.clone(), location, val, is_const);
        } else {
            let mut current = &mut self.head;

            if current.is_none() {
                println!("Empty varlist");
            }

            loop {
                if let Some(node) = current.as_mut() {
                    if node.name == varName {
                        node.value = val;
                        node.location = location;
                        node.is_const = is_const;
                        break;
                    }
                    current = &mut node.next;
                } else {
                    break;
                }
            }
        }
    }

    /*
    ***********************************************************************
      FUNCTION TO FREE THE VARIABLE INFORMATION LIST
    ************************************************************************
    */
    fn free_list(&mut self) {
        while !self.head.is_none() {
            self.head.take().map(|node| {
                self.head = node.next;
            });
        }
    }

    fn print_list(&self) {
        let mut current = &self.head;

        if current.is_none() {
            println!("Empty varlist");
        }

        loop {
            if let Some(node) = current.as_ref() {
                if !node.is_const {
                    print!("\t {} : {} -> ", node.name, node.location);
                } else {
                    print!("\t {} : {} -> ", node.value, node.location);
                }

                current = &node.next;
            } else {
                break;
            }
        }
        println!();
    }
}

/*
*************************************************************************************
   YOUR CODE IS TO BE FILLED IN THE GIVEN TODO BLANKS. YOU CAN CHOOSE TO USE ALL
   UTILITY FUNCTIONS OR NONE. YOU CAN ALSO ADD NEW FUNCTIONS
**************************************************************************************
*/
#[no_mangle]
fn init_asm(fileptr: &mut File, funcName: String) {
    fileptr
        .write_all(format!("\n.globl {}", funcName).as_bytes())
        .expect("Unable to write data");
    fileptr
        .write_all(format!("\n{}:", funcName).as_bytes())
        .expect("Unable to write data");

    // Iinitialize the stack and base pointer
    fileptr
        .write_all("\npushq %rbp".as_bytes())
        .expect("Unable to write data");
    fileptr
        .write_all("\nmovq %rsp, %rbp".as_bytes())
        .expect("Unable to write data");
}

/*
***************************************************************************
   FUNCTION TO WRITE THE RETURNING CODE OF A FUNCTION IN THE ASSEMBLY FILE
****************************************************************************
*/
#[no_mangle]
fn ret_asm(fileptr: &mut File, glb: &globals) {
    if glb.last_used_offset + 8 < 0 {
        fileptr
            .write_all(format!("\naddq ${}, %rsp  # Deallocate stack space", -(glb.last_used_offset + 8)).as_bytes())
            .expect("Failed to deallocate stack space");
    }
    fileptr
        .write_all("\npopq %rbp".as_bytes())
        .expect("Unable to write data");
    fileptr
        .write_all("\nretq\n".as_bytes())
        .expect("Unable to write data");
}

/*
***************************************************************************
  FUNCTION TO CONVERT OFFSET FROM LONG TO CHAR STRING
****************************************************************************
*/
#[no_mangle]
fn long_to_char_offset(glb: &mut globals) {
    if glb.last_used_offset == 0 {
        glb.last_used_offset -= 8;
        println!("[DEBUG] First constant stored, offset moved to: {}", glb.last_used_offset);
    }

    // glb.last_used_offset -= 8;
    println!("\n[DEBUG] Offset is now(long_to_char): {}...", glb.last_used_offset);

    glb.last_offset_used = format!("{}", glb.last_used_offset);

    // ensure no more than 100 characters are used
    if glb.last_offset_used.len() > 100 {
        glb.last_offset_used.truncate(100);
    }

    glb.last_offset_used.push_str("(%rbp)");
}

/*
***************************************************************************
  FUNCTION TO SAVE VALUE IN ACCUMULATOR (RAX)
****************************************************************************
*/
#[no_mangle]
fn save_val_rax(
    fileptr: &mut File,
    name: String,
    glb: &mut globals,
    var_list: &mut varStList,
    reg_list: &mut regList,
) {
    let temp_reg = reg_list.get_next_avail_reg(true);

    if temp_reg == "NoReg" {
        long_to_char_offset(glb);

        fileptr
            .write_all(format!("\n movq %rax, {}", glb.last_offset_used).as_bytes())
            .expect("Unable to write data");

        var_list.update_var_info(name, glb.last_offset_used.clone(), INVAL, false);
        reg_list.update_reg_info("%rax".to_string(), 1);
    } else {
        fileptr
            .write_all(format!("\nmovq %rax, {}", temp_reg).as_bytes())
            .expect("Unable to write data");

        reg_list.update_reg_info(temp_reg.clone(), 0);
        var_list.update_var_info(name, temp_reg, INVAL, false);
        reg_list.update_reg_info("%rax".to_string(), 1);
    }
}
#[no_mangle]
fn create_reg_list(reg_list: &mut regList) {
    // Create the initial reglist which can be used to store variables.
    // 4 general purpose registers : AX, BX, CX, DX
    // 4 special purpose : SP, BP, SI , DI.
    // Other registers: r8, r9
    // You need to decide which registers you will add in the register list
    // use. Can you use all of the above registers?
    /*
     ****************************************
              TODO : YOUR CODE HERE
     ***************************************
    */
    //general purpose registers
    reg_list.add_reg("%rax", 1);
    reg_list.add_reg("%rbx", 1);
    reg_list.add_reg("%rcx", 1);
    reg_list.add_reg("%rdx", 1);

    //special purpose registers
    reg_list.add_reg("%rsp", 0); //marked unavailable because it is used to track top of the stack
    reg_list.add_reg("%rbp", 0); //tracks function stack frame
    reg_list.add_reg("%rsi", 1);
    reg_list.add_reg("%rdi", 1);

    //other registers
    reg_list.add_reg("%r8", 1);
    reg_list.add_reg("%r9", 1);

}
/*
***********************************************************************
  THIS FUNCTION IS MEANT TO PUT THE FUNCTION ARGUMENTS ON STACK
************************************************************************
*/
#[no_mangle]
fn push_arg_on_stack(
    fileptr: &mut File,
    arguments: &RList,
    glb: &mut globals,
    var_list: &mut varStList,
    reg_list: &mut regList,
) {
    /*
     ****************************************
              TODO : YOUR CODE HERE
     ****************************************
    */

    let mut args = arguments;

    let mut argument_index = 0; //tracks args
    let argument_registers = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
    loop {
        /*
        ***********************************************************************
                 TODO : YOUR CODE HERE
         THINK ABOUT WHERE EACH ARGUMENT COMES FROM. EXAMPLE WHERE IS THE
         FIRST ARGUMENT OF A FUNCTION STORED.
        ************************************************************************
        */
        if let Some(node) = args.node.as_ref() {
            let argument_value = node.value;
            //used to store the reg names and stack offset
            let mut location = String::new();

            if argument_index < 6{
                location = argument_registers[argument_index].to_string();

                fileptr
                    .write_all(format!("\nmovq ${}, {}",argument_value, location).as_bytes())
                    .expect("Failed to write argument to register");
            } else{
                //push on  stack
                location = format!("-{}(%rbp)", glb.last_used_offset);

                fileptr
                    .write_all(format!("\npushq ${}", argument_value).as_bytes())
                    .expect("Failed to push argument to stack");
                    glb.last_used_offset -= 8;
                    println!("\n[DEBUG] Offset is now(push_arg_): {}...", glb.last_used_offset);

            }
            // Store argument location in variable list
            var_list.add_var_info(format!("arg{}", argument_index + 1), location, argument_value, false);

            argument_index += 1;

        } else {
            break;
        }

        if let Some(next) = args.next.as_ref() {
            args = next;
        } else {
            break;
        }
    }
}

/*
*************************************************************************
  THIS FUNCTION IS MEANT TO GET THE FUNCTION ARGUMENTS FROM THE  STACK
**************************************************************************
*/
#[no_mangle]
fn pop_arg_from_stack(
    fileptr: &mut File,
    arguments: &RList,
    glb: &mut globals,
    var_list: &mut varStList,
    reg_list: &mut regList,
) {
    let mut args = arguments;
    let argument_registers = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

    let mut argument_index = 0;
    /*
     ****************************************
              TODO : YOUR CODE HERE
     ****************************************
    */

    loop {
        /*
        ***********************************************************************
                 TODO : YOUR CODE HERE
         THINK ABOUT WHERE EACH ARGUMENT COMES FROM. EXAMPLE WHERE IS THE
         FIRST ARGUMENT OF A FUNCTION STORED AND WHERE SHOULD IT BE EXTRACTED
         FROM AND STORED TO..
        ************************************************************************
        */
        if let Some(node) = args.node.as_ref() {
            let mut location = String::new();

            if argument_index <argument_registers.len(){
                location = argument_registers[argument_index].to_string();
            }else{
                if glb.last_used_offset < 0{
                    glb.last_used_offset += 8;
                    println!("\n[DEBUG] Offset is now(pop_arg): {}...", glb.last_used_offset);
                    location = format!("{}(%rbp)", glb.last_used_offset);
                }
            }

            var_list.add_var_info(node.name.clone(), location.clone(), INVAL, false);

            // Debug output to see what’s happening
            // fileptr
            //     .write_all(format!("\n# Pop arg{} from {}\n", argument_index + 1, location).as_bytes())
            //     .expect("Failed to write argument retrieval to assembly");

            argument_index += 1;
        } else {
            break;
        }

        if let Some(next) = args.next.as_ref() {
            args = next;
        } else {
            break;
        }
    }
}
/*
***************************************************************************
  FUNCTION TO CONVERT CONSTANT VALUE TO CHAR STRING
****************************************************************************
*/
#[no_mangle]
fn process_constant(
    fileptr: &mut File,
    op_node: &RNode,
    glb: &mut globals,
    var_list: &mut varStList,
) {
    long_to_char_offset(glb);

    let mut value = format!("{}", op_node.value);
    if value.len() > 10 {
        value.truncate(10);
    }

    let mut offset = format!("{}", glb.last_used_offset);
    if offset.len() > 100 {
        offset.truncate(100);
    }
    offset.push_str("(%rbp)");

    var_list.add_var_info("".to_string(), offset.clone(), op_node.value, true);

    fileptr
        .write_all(format!("\nmovq ${}, {}", value, offset).as_bytes())
        .expect("Unable to write data");
}

/*
***********************************************************************
 THIS FUNCTION IS MEANT TO PROCESS EACH CODE STATEMENT AND GENERATE
 ASSEMBLY FOR IT.
 TIP: YOU CAN MODULARIZE BETTER AND ADD NEW SMALLER FUNCTIONS IF YOU
 WANT THAT CAN BE CALLED FROM HERE.
************************************************************************
*/
#[no_mangle]
fn process_statements(
    fileptr: &mut File,
    statements: &RList,
    glb: &mut globals,
    var_list: &mut varStList,
    reg_list: &mut regList,
) {
    let mut stmt = statements;
    let mut local_vars = vec![];
    let mut last_temp_location: Option<String> = None; // Track last temp location

    println!("\n[DEBUG] Counting unique variables for stack allocation...");

    // **Step 1: Count unique variables**
    loop {
        if let Some(node) = stmt.node.as_ref() {
            if node.stmtCode == StmtType::ASSIGN {
                println!("\n[DEBUG] Okay, assignment {}...", node.name);
                let variable_name = node.name.clone();
                if !local_vars.contains(&variable_name) {
                    println!("[DEBUG] Found variable for stack: {}", variable_name);
                    local_vars.push(variable_name.clone());
                } else {
                    println!("\n[DEBUG] Variable already allocated...");
                }
            }
        }

        if let Some(next) = stmt.next.as_ref() {
            stmt = next;
        } else {
            break;
        }
    }

    // **Step 2: Allocate Stack Space for Local Variables**
    let stack_size = local_vars.len() as i64 * 8; // Each variable takes 8 bytes
    if stack_size > 0 {
        println!("[DEBUG] Allocating {} bytes on the stack for local variables.", stack_size);
        fileptr
            .write_all(format!("\nsubq ${}, %rsp  # Allocate stack space", stack_size).as_bytes())
            .expect("Failed to allocate stack space");
    }

    stmt = statements;
    println!("[DEBUG] Processing statements...");

    loop {
        if let Some(node) = stmt.node.as_ref() {
            match node.stmtCode {
                StmtType::ASSIGN => {
                    println!("[DEBUG] Processing ASSIGN statement...");

                    if let Some(right) = node.right.as_ref() {
                        println!("[DEBUG] Processing right-hand side of ASSIGN...");
                        process_expression(fileptr, right, glb, var_list, reg_list);
                    }

                    let variable_name = node.name.clone(); // The assigned variable
                    println!("[DEBUG] Storing variable '{}' into memory.", variable_name);

                    if glb.last_used_offset == 0{
                        glb.last_used_offset -= 8;
                    }
                   // ✅ **Always allocate new stack space for the variable**
                    let stack_location = format!("{}(%rbp)", glb.last_used_offset);
                    println!("[DEBUG] Allocated stack space for '{}': {}", variable_name, stack_location);
                    glb.last_used_offset -= 8;
                    println!("[DEBUG]offset is now: {}", glb.last_used_offset);

                    // ✅ **Update variable storage**
                    var_list.add_var_info(variable_name.clone(), stack_location.clone(), INVAL, false);

                    // ✅ **Store result in memory**
                    fileptr
                        .write_all(format!("\nmovq %rax, {}", stack_location).as_bytes())
                        .expect("Failed to store variable in memory");
                }

                StmtType::RETURN => {
                    println!("[DEBUG] Processing RETURN statement...");

                    if let Some(val_left) = node.left.as_ref() {
                        println!("[DEBUG] Processing return value {}...", val_left.name);
                
                        if val_left.exprCode == ExprType::CONSTANT {
                            // ✅ Directly load constant into %rax
                            fileptr
                                .write_all(format!("\nmovq ${}, %rax", val_left.value).as_bytes())
                                .expect("Failed to write return constant");
                        } else {
                            // Process expressions or variables normally
                            process_expression(fileptr, val_left, glb, var_list, reg_list);
                        }
                    }
                
                    println!("[DEBUG] Return statement processed. Generating return sequence...");
                    ret_asm(fileptr,glb);

                    // // Store return value in %rax
                    // fileptr
                    //     .write_all(format!("\nmovq {}, %rax", glb.last_offset_used).as_bytes())
                    //     .expect("Failed to write to return statement");

                    // println!("[DEBUG] Return statement processed. Generating return sequence...");
                    // ret_asm(fileptr, glb);
                }

                StmtType::S_NONE => {
                    println!("[DEBUG] Encountered an empty statement (S_NONE). Skipping.");
                }
            }
        } else {
            break;
        }

        if let Some(next) = stmt.next.as_ref() {
            stmt = next;
        } else {
            break;
        }
    }

    println!("[DEBUG] Finished processing statements.");
}



#[no_mangle]
fn process_expression(
    fileptr: &mut File,
    expression_node: &RNode,
    glb: &mut globals,
    var_list: &mut varStList,
    reg_list: &mut regList,
) {
    match expression_node.exprCode {
        ExprType::VARIABLE => {
            println!("\n[DEBUG] Processing expr_VARIABLE: {}", expression_node.name);
            
            let variable_location = var_list.lookup_var_info(expression_node.name.clone(), INVAL);
            if variable_location.is_empty() {
                panic!("Error: Variable {} not found!", expression_node.name);
            }

            fileptr
                .write_all(format!("\nmovq {}, %rax", variable_location).as_bytes())
                .expect("Failed to load variable");
        }

        ExprType::CONSTANT => {
            println!("\n[DEBUG] Processing expr_CONSTANT: {}", expression_node.value);
            process_constant(fileptr, expression_node, glb, var_list);
        }

        ExprType::OPERATION => {
            println!("\n[DEBUG] Processing OPERATION: {:?}", expression_node.opCode);

            let mut left_location = String::new();
            let mut right_location = String::new();

            if let Some(left_expr) = expression_node.left.as_ref() {
                left_location = var_list.lookup_var_info(left_expr.name.clone(), INVAL);
                if left_location.is_empty() {
                    process_expression(fileptr, left_expr, glb, var_list, reg_list);
                    left_location = glb.last_offset_used.clone();
                }
            }

            if let Some(right_expr) = expression_node.right.as_ref() {
                right_location = var_list.lookup_var_info(right_expr.name.clone(), INVAL);
                if right_location.is_empty() {
                    process_expression(fileptr, right_expr, glb, var_list, reg_list);
                    right_location = glb.last_offset_used.clone();
                }
            }
            match expression_node.opCode {
                // ✅ Handle Function Calls
                OpType::FUNCTIONCALL => {  
                    if let Some(left) = expression_node.left.as_ref() {
                        println!("\n[DEBUG] Processing function call: {}", left.name);
                    }
                    
                    
                    let mut arg_count = 0;
                    let mut args = expression_node.arguments.as_ref();  
                    let arg_registers = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
            
                    while let Some(arg_list) = args {  
                        if let Some(arg) = arg_list.node.as_ref() {  
                            let mut arg_location = var_list.lookup_var_info(arg.name.clone(), INVAL);
                            if arg_location.is_empty() {
                                process_expression(fileptr, arg, glb, var_list, reg_list);
                                arg_location = glb.last_offset_used.clone();
                            }
            
                            if arg_count < 6 {
                                if arg_location != arg_registers[arg_count]{
                                    fileptr
                                    .write_all(format!("\nmovq {}, {}", arg_location, arg_registers[arg_count]).as_bytes())
                                    .expect("Failed to pass function argument in register");
                                }
                                
                            } else {
                                fileptr
                                    .write_all(format!("\npushq {}", arg_location).as_bytes())
                                    .expect("Failed to push function argument to stack");
                            }
            
                            arg_count += 1;
                        }
            
                        args = arg_list.next.as_ref();
                    }
                    if let Some(left) = expression_node.left.as_ref() {
                        fileptr
                            .write_all(format!("\ncall {}", left.name).as_bytes())
                            .expect("Failed to generate function call");
                
                        if arg_count > 6 {
                            let stack_cleanup = (arg_count - 6) * 8;
                            fileptr
                                .write_all(format!("\naddq ${}, %rsp  # Restore stack", stack_cleanup).as_bytes())
                                .expect("Failed to restore stack after function call");
                        }
                    }
            
                    long_to_char_offset(glb);
                    // fileptr
                    //     .write_all(format!("\nmovq %rax, {}", glb.last_offset_used).as_bytes())
                    //     .expect("Failed to store function return value");
            
                    // var_list.update_var_info(expression_node.name.clone(), glb.last_offset_used.clone(), INVAL, false);
                }

                // ✅ Handle Arithmetic Operations (Multiplication, Division, Addition, Subtraction)
                OpType::MULTIPLY | OpType::DIVIDE | OpType::ADD | OpType::SUBTRACT => {  
                    let mut left_location = String::new();
                    let mut right_location = String::new();
            
                    if let Some(left_expr) = expression_node.left.as_ref() {
                        left_location = var_list.lookup_var_info(left_expr.name.clone(), INVAL);
                        if left_location.is_empty() {
                            process_expression(fileptr, left_expr, glb, var_list, reg_list);
                            left_location = glb.last_offset_used.clone();
                        }
                    }
            
                    if let Some(right_expr) = expression_node.right.as_ref() {
                        right_location = var_list.lookup_var_info(right_expr.name.clone(), INVAL);
                        if right_location.is_empty() {
                            process_expression(fileptr, right_expr, glb, var_list, reg_list);
                            right_location = glb.last_offset_used.clone();
                        }
                    }
            
                    if expression_node.opCode == OpType::DIVIDE {
                        // ✅ Corrected `idivq` handling to use `%rax` as the dividend and a single operand
                        fileptr
                            .write_all(format!("\nmovq {}, %rax", left_location).as_bytes())  // ✅ Load numerator into %rax
                            .expect("Failed to load numerator");

                        fileptr
                            .write_all("\ncqto".as_bytes())  // ✅ Sign-extend RAX into RDX:RAX
                            .expect("Failed to sign-extend for division");

                        fileptr
                            .write_all(format!("\nidivq {}", right_location).as_bytes())  // ✅ Correct: idivq only takes one operand
                            .expect("Failed to generate div operation");
                    } else {
                        // ✅ Handle all other arithmetic operations
                        let operation_instr = match expression_node.opCode {
                            OpType::ADD => "addq",
                            OpType::SUBTRACT => "subq",
                            OpType::MULTIPLY => "imulq",
                            _ => unreachable!(),
                        };
            
                        fileptr
                            .write_all(format!("\nmovq {}, %rax", left_location).as_bytes())
                            .expect("Failed to load left operand");

                        fileptr
                            .write_all(format!("\n{} {}, %rax", operation_instr, right_location).as_bytes())
                            .expect("Failed to generate arithmetic operation");
                    }
                }
                OpType::BOR => {
                    fileptr
                        .write_all(format!("\nmovq {}, %rax", left_location).as_bytes())
                        .expect("Failed to load left operand");
                    fileptr
                        .write_all(format!("\norq {}, %rax", right_location).as_bytes())
                        .expect("Failed to generate OR");
                }
                OpType::BAND => {
                    fileptr
                        .write_all(format!("\nmovq {}, %rax", left_location).as_bytes())
                        .expect("Failed to load left operand");
                    fileptr
                        .write_all(format!("\nandq {}, %rax", right_location).as_bytes())
                        .expect("Failed to generate AND");
                }
                OpType::BXOR => {
                    fileptr
                        .write_all(format!("\nmovq {}, %rax", left_location).as_bytes())
                        .expect("Failed to load left operand");
                    fileptr
                        .write_all(format!("\nxorq {}, %rax", right_location).as_bytes())
                        .expect("Failed to generate XOR");
                }

                // ✅ **Bit Shifting**
                OpType::BSHL => {
                    fileptr
                        .write_all(format!("\nmovq {}, %rcx", right_location).as_bytes())
                        .expect("Failed to load shift amount");
                    fileptr
                        .write_all("\nshlq %cl, %rax".as_bytes())
                        .expect("Failed to generate Left Shift");
                }
                OpType::BSHR => {
                    fileptr
                        .write_all(format!("\nmovq {}, %rcx", right_location).as_bytes())
                        .expect("Failed to load shift amount");
                    fileptr
                        .write_all("\nsarq %cl, %rax".as_bytes())
                        .expect("Failed to generate Right Shift");
                }

                // ✅ **Unary Negation**
                OpType::NEGATE => {
                    fileptr
                        .write_all("\nnegq %rax".as_bytes())
                        .expect("Failed to generate NEG");
                }
                
            
                _ => {  
                    println!("[WARNING] Unhandled operation type: {:?}", expression_node.opCode);
                }
            }
        }

        _ => {
            println!("[WARNING] Unsupported expression type: {:?}", expression_node.exprCode);
        }
    }
}


        
/*
 ***********************************************************************
  THIS FUNCTION IS MEANT TO DO CODEGEN FOR ALL THE FUNCTIONS IN THE FILE
 ************************************************************************
*/
#[no_mangle]
pub fn Codegen(mut worklist: &RList) {
    /*
     ****************************************
              TODO : YOUR CODE HERE
     ****************************************
    */

    //creates output assembly file
    let mut fileptr = File::create("assembly.s").expect("Unable to create assembly file");

    // // Initialize register list
    let mut reg_list = regList::new();
    // create_reg_list(&mut reg_list);

    // // Print register list to check if it's working
    // println!("Register List After Initialization:");
    // reg_list.print_list();
    
    // Loop through each function in the program
    loop {
        if let Some(node) = worklist.node.as_ref() {
            if node.type_ == NodeType::FUNCTIONDECL {
                // Get function name
                let func_name = node.name.clone();
                println!("Generating assembly for function: {}", func_name);

                // Initialize function prologue
                init_asm(&mut fileptr, func_name.clone());

                // Initialize global variables
                let mut glb = globals::new();

                // Initialize variable storage list
                let mut var_list = varStList::new();

                // Process function parameters (if any)
                if let Some(arguments) = node.arguments.as_ref() {
                    pop_arg_from_stack(&mut fileptr, arguments, &mut glb, &mut var_list, &mut reg_list);
                }

                // Process function body statements
                if let Some(statements) = node.statements.as_ref() {
                    process_statements(&mut fileptr, statements, &mut glb, &mut var_list, &mut reg_list);
                }

                // Add function epilogue (return)
                // ret_asm(&mut fileptr);
            }
        } else {
            break;
        }

        // Move to the next function in the list
        if let Some(next) = worklist.next.as_ref() {
            worklist = next;
        } else {
            break;
        }
    }

    println!("Assembly code successfully written to 'assembly.s'");
}

/*
**********************************************************************************************************************************
 YOU CAN MAKE ADD AUXILLIARY FUNCTIONS BELOW THIS LINE. DO NOT FORGET TO DECLARE THEM IN THE HEADER
**********************************************************************************************************************************
*/

/*
**********************************************************************************************************************************
 YOU CAN MAKE ADD AUXILLIARY FUNCTIONS ABOVE THIS LINE. DO NOT FORGET TO DECLARE THEM IN THE HEADER
**********************************************************************************************************************************
*/
